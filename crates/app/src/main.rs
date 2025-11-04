fn main() {
    let mut path = std::env::current_dir().unwrap();
    path.push("crates\\app\\src\\patrick.mp4");
    let video = media::frame::streams::Video::new(path).expect("open video");
    let producer =
        media::frame::Producer::new(video, media::frame::streams::OnStreamEnd::HoldLastFrame)
            .expect("create producer");

    // per wgpu error message I got:
    // Initializing the event loop outside of the main thread is a significant cross-platform compatibility hazard.
    // If you absolutely need to create an EventLoop on a different thread, you can use the `EventLoopBuilderExtWindows::any_thread` function.
    // let app_thread = thread::spawn(move || {
    //     if let Err(e) = engine::run(producer) {
    //         eprintln!("engine failed: {e:?}");
    //     }
    // });

    if let Err(e) = engine::run(producer) {
        eprintln!("engine failed: {e:?}");
    }
}
