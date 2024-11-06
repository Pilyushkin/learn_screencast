use std::os::fd::AsRawFd;

use gbm;

const WIDTH: u32 = 4;
const HEIGHT: u32 = 4;

fn main() {
    // let Some(fd) = std::env::args().nth(1) else {
    //     println!("Expected fd");
    //     return;
    // };

    let fd = read_fd();
    println!("Received fd: {fd}");

    // let buffer_fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd.parse::<i32>().unwrap()) };
    let buffer_fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };

    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true);
    let drm_fd = options
        .open("/dev/dri/card0")
        .or(options.open("/dev/dri/card1"))
        .unwrap();
    let gbm = gbm::Device::new(drm_fd).unwrap();

    let bo = gbm
        .import_buffer_object_from_dma_buf_with_modifiers::<()>(
            1,
            [Some(buffer_fd), None, None, None],
            WIDTH,
            HEIGHT,
            gbm::Format::Argb8888,
            gbm::BufferObjectFlags::empty(),
            [256; 4],
            [0; 4],
            gbm::Modifier::Linear,
        )
        .unwrap();

    let _ = bo
        .map(&gbm, 0, 0, WIDTH, HEIGHT, |mbo| {
            println!("Buffer size: {}", mbo.buffer().len());
            println!("Buffer: {:?}", mbo.buffer());
        })
        .unwrap()
        .unwrap();
}

fn read_fd() -> std::os::fd::RawFd {
    use passfd::FdPassingExt;
    use std::os::unix::net::UnixStream;

    let stream = UnixStream::connect("/tmp/dma_buf_transfer.sock").unwrap();
    let fd = stream.recv_fd().unwrap();
    fd
}
