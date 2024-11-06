use std::os::fd::AsRawFd;

use drm::Device;
use gbm;

use std::os::unix::io::AsFd;
use std::os::unix::io::BorrowedFd;

#[derive(Debug)]
/// A simple wrapper for a device node.
struct Card(std::fs::File);

/// Implementing [`AsFd`] is a prerequisite to implementing the traits found
/// in this crate. Here, we are just calling [`File::as_fd()`] on the inner
/// [`File`].
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

/// With [`AsFd`] implemented, we can now implement [`drm::Device`].
impl Device for Card {}

impl Card {
    /// Simple helper method for opening a [`Card`].
    fn open() -> Self {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options.write(true);

        // The normal location of the primary device node on Linux
        let fd = options
            .open("/dev/dri/card0")
            .or(options.open("/dev/dri/card1"))
            .unwrap();
        Card(fd)
    }
}

fn main() {
    let drm_fd = Card::open();
    let gbm = gbm::Device::new(drm_fd).unwrap();

    const WIDTH: u32 = 4;
    const HEIGHT: u32 = 4;

    println!(
        "prime: {}",
        gbm.get_driver_capability(drm::DriverCapability::Prime)
            .unwrap()
    );

    let flags = {
        use gbm::BufferObjectFlags as f;
        /*f::CURSOR | f::WRITE | f::SCANOUT*/
        f::empty()
    };

    for format in [gbm::Format::Bgra8888, gbm::Format::Argb8888] {
        println!("{format}: {}", gbm.is_format_supported(format, flags));
    }

    let modifers = &[
        gbm::Modifier::Linear,
        gbm::Modifier::Nvidia_16bx2_block_four_gob,
    ];
    let mut bo = gbm
        .create_buffer_object_with_modifiers2::<()>(
            WIDTH,
            HEIGHT,
            gbm::Format::Argb8888,
            modifers.iter().cloned(),
            flags,
        )
        .unwrap();

    println!(
        "plane_count: {}\nstride: {}\noffset: {}",
        bo.plane_count().unwrap(),
        bo.stride().unwrap(),
        bo.offset(0).unwrap()
    );

    let _ = bo
        .map_mut(&gbm, 0, 0, WIDTH, HEIGHT, |mbo| {
            let b = mbo.buffer_mut();
            for i in 0..WIDTH {
                for j in 0..HEIGHT {
                    let idx = (i * WIDTH + j) as usize;
                    b[idx] = if i % 2 == 0 { 0 } else { 255 };
                }
            }
            println!("buf size: {}", b.len());
            // println!("send: {:?}", b);
        })
        .unwrap()
        .unwrap();

    let export_fd = bo.fd().unwrap();

    println!(
        "fd: {}, modifier: {:?}",
        export_fd.as_raw_fd(),
        bo.modifier().unwrap()
    );

    use passfd::FdPassingExt;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::UnixListener;

    let _ = std::fs::remove_file("/tmp/dma_buf_transfer.sock");
    let listener = UnixListener::bind("/tmp/dma_buf_transfer.sock").unwrap();

    loop {
        let (stream, _) = listener.accept().unwrap();
        stream.send_fd(export_fd.as_raw_fd()).unwrap();
    }

    // println!("sleeping...");
    // std::thread::sleep(std::time::Duration::from_secs(60 * 60 * 13));
}
