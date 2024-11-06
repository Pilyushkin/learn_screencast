// Note https://github.com/Genymobile/scrcpy/issues/4507 (loop v4l2 not working)

use portal_screencast as psc;
use screencast::pipewire_stream::PipewireStream;
use std::cell::RefCell;
use std::os::fd::FromRawFd;
use std::rc::Rc;

slint::include_modules!();

fn main() {
    let ui = Ui::new().unwrap();
    let active_screen_cast: Rc<RefCell<Option<psc::ActiveScreenCast>>> =
        Rc::new(RefCell::new(None));
    let mut pw_stream = PipewireStream::create();
    let weak_ui = ui.as_weak();
    ui.on_start({
        let active_screen_cast = Rc::clone(&active_screen_cast);
        move |on| {
            if on {
                let mut screen_cast = psc::ScreenCast::new().unwrap();
                // Set which source types to allow, and enable multiple items to be shared.
                screen_cast.set_source_types(psc::SourceType::MONITOR | psc::SourceType::WINDOW);
                // screen_cast.enable_multiple();
                // If you have a window handle you can tie the dialog to it
                if let Ok(screen_cast) = screen_cast.start(None) {
                    let pw_fd =
                        unsafe { std::os::fd::OwnedFd::from_raw_fd(screen_cast.pipewire_fd()) };
                    let stream_id = screen_cast.streams().next().unwrap().pipewire_node();
                    let frame_receiver = pw_stream.start(pw_fd, stream_id);
                    slint::spawn_local({
                        let weak_ui = weak_ui.clone();
                        async move {
                            while let Ok(frame) = frame_receiver.recv().await {
                                weak_ui
                                    .upgrade()
                                    .unwrap()
                                    .set_frame(slint::Image::from_rgba8(frame));
                            }
                            weak_ui
                                .upgrade()
                                .unwrap()
                                .set_frame(slint::Image::default());
                        }
                    })
                    .unwrap();
                    *active_screen_cast.borrow_mut() = Some(screen_cast);
                }
            } else {
                pw_stream.stop();
                *active_screen_cast.borrow_mut() = None;
            }
        }
    });
    ui.run().unwrap();
}
