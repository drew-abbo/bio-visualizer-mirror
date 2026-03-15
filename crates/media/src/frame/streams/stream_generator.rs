//! Exports [StreamGenerator].

use std::collections::VecDeque;
use std::convert::Infallible;

use util::channels::message_channel::Outbox;
use util::channels::request_channel::{ReqRes, Server};
use util::channels::{ChannelError, ChannelResult};

use crate::fps::Fps;
use crate::playback_stream::BufferingSuggestor;

/// A utility for creating streams that will generate and send data ahead of
/// time to another thread.
pub trait StreamGenerator: Sized + Send {
    /// The data that the stream generates and sends.
    type Data: Send;

    /// A server request.
    type Request: Send;

    /// The response to server requests.
    type Response: Send;

    /// A note for [Self::handle_invalid_queue] that tells it what is wrong with
    /// the queue and how to fix it.
    type QueueInvalidNote: Send;

    /// Starts up a worker for the [StreamGenerator] that will generate/send
    /// data and handle requests. *This function is intended to be run on
    /// another thread*.
    ///
    /// This function returns when either the `frame_outbox`'s
    /// [Inbox](util::channels::message_channel::Inbox) or the
    /// `request_server`'s [Client](util::channels::request_channel::Client) is
    /// dropped.
    fn run(
        self,
        data_outbox: Outbox<Self::Data>,
        request_server: Server<Self::Request, Self::Response>,
    ) {
        _ = StreamGeneratorOuter::new(self, data_outbox, request_server).run();
    }

    /// The stream's target frame rate.
    fn target_fps(&self) -> Fps;

    /// Generate a new piece of data to send.
    ///
    /// The number of data items already in flight (sent but not received) is
    /// provided as `in_flight`. This value will have been fetched immediately
    /// before this is called.
    fn new_data(&mut self, in_flight: usize) -> Self::Data;

    /// Handle a request.
    ///
    /// If [Some] is returned with an inner [Self::QueueInvalidNote],
    /// [Self::handle_invalid_queue] will be called with the inner
    /// [Self::QueueInvalidNote] immediately after this function returns.
    ///
    /// [Self::create_response_for_request] will be called with the same `req`
    /// if a response is being waited on. This happens after the cache has been
    /// fixed (if it needed to be).
    fn handle_request(&mut self, req: &mut Self::Request) -> Option<Self::QueueInvalidNote>;

    /// Handle an invalidated queue with a [Self::QueueInvalidNote] that was
    /// returned from [Self::handle_request] when handling a request `req`.
    fn handle_invalid_queue(
        &mut self,
        queue: &mut VecDeque<Self::Data>,
        req: &mut Self::Request,
        queue_invalid_note: Self::QueueInvalidNote,
    );

    /// Generate a response for a request `req` (already handled by
    /// [Self::handle_request] and [Self::handle_invalid_queue]).
    fn create_response_for_request(&mut self, req: Self::Request) -> Self::Response;
}

struct StreamGeneratorOuter<T: StreamGenerator> {
    generator: T,
    data_outbox: Outbox<T::Data>,
    request_server: Server<T::Request, T::Response>,
}

impl<T: StreamGenerator> StreamGeneratorOuter<T> {
    /// Create a [RunStreamGeneratorInner].
    pub const fn new(
        generator: T,
        data_outbox: Outbox<T::Data>,
        request_server: Server<T::Request, T::Response>,
    ) -> Self {
        Self {
            generator,
            data_outbox,
            request_server,
        }
    }

    /// Runs the stream generator, sending and handling requests.
    pub fn run(mut self) -> ChannelResult<Infallible> {
        let mut buffering_suggestor = BufferingSuggestor::new(self.generator.target_fps());
        let mut in_flight = 0;

        loop {
            let new_data =
                buffering_suggestor.run_timed_and_sampled(|| self.generator.new_data(in_flight));

            buffering_suggestor.set_dest_fps(self.generator.target_fps());
            let buffering_suggestion = buffering_suggestor.buffering_suggestion();

            let new_data = match self
                .data_outbox
                .send_bounded(new_data, buffering_suggestion)
            {
                Ok(_) => {
                    in_flight = self.handle_requests()?;
                    continue;
                }
                Err(ChannelError::SendBlocked { msg }) => msg,
                Err(e) => return Err(e.unmap_msg()),
            };
            // If we got send-blocked it means the client needs us to deal with
            // something before they can take more frames.

            // We'll force the data onto the end of the queue so that if the
            // queue gets invalidated while we're responding to requests the
            // invalid queue handler can see *every* item that has been
            // generated but not received.
            self.data_outbox
                .with_queue_in_place(|queue| queue.push_back(new_data));

            in_flight = self.handle_requests()?;
        }
    }

    fn handle_requests(&mut self) -> ChannelResult<usize> {
        let mut handle_all_requests = |requests: &mut VecDeque<ReqRes<_, _>>| -> () {
            for (mut request, res_handle) in requests.drain(..) {
                let queue_invalid_note = self.generator.handle_request(&mut request);

                if let Some(queue_invalid_note) = queue_invalid_note {
                    self.data_outbox.with_queue_in_place(|queue| {
                        self.generator.handle_invalid_queue(
                            queue,
                            &mut request,
                            queue_invalid_note,
                        );
                    });
                }

                if let Some(res_handle) = res_handle {
                    let response = self.generator.create_response_for_request(request);
                    _ = res_handle.respond(response);
                }
            }
        };

        let in_flight = self
            .request_server
            .check_in_place(|queue| {
                handle_all_requests(queue);
                queue.len()
            })?
            .unwrap_or(0);

        Ok(in_flight)
    }
}
