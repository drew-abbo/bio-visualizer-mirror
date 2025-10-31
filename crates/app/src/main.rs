fn main() {
    let mut path = std::env::current_dir().unwrap();
    path.push("app\\src\\rick.mp4");
    println!("Opening video at path: {:?}", path);
    let video = media::frame::streams::Video::new(path).expect("open video");
    let producer =
        media::frame::Producer::new(video, media::frame::streams::OnStreamEnd::HoldLastFrame)
            .expect("create producer");

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
