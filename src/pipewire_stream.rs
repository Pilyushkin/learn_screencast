use std::os::fd::OwnedFd;
use std::thread::JoinHandle;

pub struct PipewireStream {
    thread_handle: Option<JoinHandle<()>>,
    cmd_sender: Option<pipewire::channel::Sender<inner::Command>>,
}

impl PipewireStream {
    pub fn create() -> Self {
        Self {
            thread_handle: None,
            cmd_sender: None,
        }
    }

    pub fn start(
        &mut self,
        pipewire_fd: OwnedFd,
        stream_id: u32,
    ) -> async_channel::Receiver<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
        let (frame_sender, frame_receiver) = async_channel::bounded(10);
        let (cmd_sender, cmd_receiver) = pipewire::channel::channel();
        self.thread_handle = Some(std::thread::spawn(move || {
            inner::pipewire_thread(pipewire_fd, stream_id, frame_sender, cmd_receiver).unwrap()
        }));
        self.cmd_sender = Some(cmd_sender);
        frame_receiver
    }

    pub fn stop(&mut self) {
        self.cmd_sender
            .take()
            .unwrap()
            .send(inner::Command::Stop)
            .unwrap();
        self.thread_handle.take().unwrap().join().unwrap();
    }
}

mod inner {
    use crate::egl_dma_buf as dma;
    use pipewire::spa;
    use pipewire::{self as pw, context::Context, main_loop::MainLoop, properties::properties};
    use std::cell::RefCell;
    use std::os::fd::OwnedFd;
    use std::rc::Rc;

    #[derive(Debug)]
    pub enum Command {
        Stop,
    }

    pub fn pipewire_thread(
        pipewire_fd: OwnedFd,
        stream_id: u32,
        frame_sender: async_channel::Sender<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
        pw_receiver: pipewire::channel::Receiver<Command>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            println!(
                "Library version: {}",
                std::ffi::CStr::from_ptr(pw::sys::pw_get_library_version())
                    .to_str()
                    .unwrap()
            );
        }

        let mainloop = Rc::new(MainLoop::new(None)?);
        let context = Context::new(&*mainloop)?;
        let core = context.connect_fd(pipewire_fd, None)?;

        let stream_data = RefCell::new(Some(start_stream(core, frame_sender, stream_id)?));

        let _receiver = pw_receiver.attach(mainloop.loop_(), {
            let mainloop = Rc::clone(&mainloop);
            move |cmd| match cmd {
                Command::Stop => {
                    (*stream_data.borrow_mut()) = None;
                    mainloop.quit();
                }
            }
        });

        mainloop.run();
        Ok(())
    }

    struct UserData {
        format: spa::param::video::VideoInfoRaw,
        dma_buf: dma::EglDmaBuf,
    }

    struct StreamData {
        _stream: pw::stream::Stream,
        _stream_listener: pw::stream::StreamListener<Rc<RefCell<UserData>>>,
    }

    fn start_stream(
        core: pipewire::core::Core,
        frame_sender: async_channel::Sender<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
        target: u32,
    ) -> Result<StreamData, pw::Error> {
        let data = Rc::new(RefCell::new(UserData {
            format: Default::default(),
            dma_buf: dma::EglDmaBuf::new().unwrap(),
        }));

        let stream = pipewire::stream::Stream::new(
            &core,
            "video-test",
            /*properties! {
                // *pw::keys::TARGET_OBJECT => target.to_string(),
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Camera",
            },*/
            properties! {"pipewire.client.reuse" => "1"},
        )?;

        let stream_listener = stream
            .add_local_listener_with_user_data(data.clone())
            .state_changed(|_, _, old, new| {
                println!("State changed: {:?} -> {:?}", old, new);
            })
            .param_changed(|_stream, user_data, id, param| {
                let Some(param) = param else {
                    return;
                };
                if id != pw::spa::param::ParamType::Format.as_raw() {
                    return;
                }

                let (media_type, media_subtype) =
                    match pw::spa::param::format_utils::parse_format(param) {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                if media_type != pw::spa::param::format::MediaType::Video
                    || media_subtype != pw::spa::param::format::MediaSubtype::Raw
                {
                    return;
                }

                user_data
                    .borrow_mut()
                    .format
                    .parse(param)
                    .expect("Failed to parse param changed to VideoInfoRaw");

                let user_data = user_data.borrow();
                println!("got video format:");
                println!(
                    "  format: {} ({:?})",
                    user_data.format.format().as_raw(),
                    user_data.format.format()
                );
                println!(
                    "  size: {}x{}",
                    user_data.format.size().width,
                    user_data.format.size().height
                );
                println!(
                    "  framerate: {}/{}",
                    user_data.format.framerate().num,
                    user_data.format.framerate().denom
                );
                println!("  format flags: {:?}", user_data.format.flags());
                println!("  modifier: {}", user_data.format.modifier());

                unsafe {
                    spa::sys::spa_debug_format(2, std::ptr::null(), param.as_raw_ptr());
                }
                /*
                let stride = user_data.format.size().width * 4 / 4;
                let size = user_data.format.size().height * stride;

                // let buffer_types = spa::pod:: // todo find property PodObject
                let object = spa::pod::Object {
                    type_: spa::sys::SPA_TYPE_OBJECT_ParamBuffers,
                    id: spa::sys::SPA_PARAM_Buffers,
                    properties: vec![
                        spa::pod::Property::new(
                            spa::sys::SPA_PARAM_BUFFERS_buffers,
                            spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
                                spa::utils::ChoiceFlags::empty(),
                                spa::utils::ChoiceEnum::Range {
                                    default: 8,
                                    min: 2,
                                    max: 64,
                                },
                            ))),
                        ),
                        spa::pod::Property::new(
                            spa::sys::SPA_PARAM_BUFFERS_blocks,
                            spa::pod::Value::Int(1),
                        ),
                        spa::pod::Property::new(
                            spa::sys::SPA_PARAM_BUFFERS_size,
                            spa::pod::Value::Int(size as i32),
                        ),
                        spa::pod::Property::new(
                            spa::sys::SPA_PARAM_BUFFERS_stride,
                            spa::pod::Value::Int(stride as i32),
                        ),
                        spa::pod::Property::new(
                            spa::sys::SPA_PARAM_BUFFERS_dataType,
                            spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
                                spa::utils::ChoiceFlags::empty(),
                                spa::utils::ChoiceEnum::Flags {
                                    default: 1 << spa::buffer::DataType::DmaBuf.as_raw() as i32,
                                    flags: vec![
                                        1 << spa::buffer::DataType::DmaBuf.as_raw() as i32,
                                        1 << spa::buffer::DataType::MemFd.as_raw() as i32,
                                        1 << spa::buffer::DataType::MemPtr.as_raw() as i32,
                                    ],
                                },
                            ))),
                        ),
                    ],
                };

                let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
                    std::io::Cursor::new(Vec::new()),
                    &pw::spa::pod::Value::Object(object),
                )
                .unwrap()
                .0
                .into_inner();

                let pod = Pod::from_bytes(&values).unwrap();

                stream.update_params(&mut [pod]).unwrap();*/
            })
            .process(move |stream, user_data| {
                let mut last_buffer: Option<pw::buffer::Buffer> = None;
                while let Some(next_buffer) = stream.dequeue_buffer() {
                    last_buffer = Some(next_buffer);
                }

                match last_buffer {
                    None => println!("out of buffers"),
                    Some(mut buffer) => {
                        let datas = buffer.datas_mut();
                        if datas.is_empty() {
                            return;
                        }

                        let user_data = user_data.borrow();

                        let buffer = if datas[0].type_() == spa::buffer::DataType::DmaBuf {
                            let mut fds = Vec::with_capacity(datas.len());
                            let mut offsets = Vec::with_capacity(datas.len());
                            let mut strides = Vec::with_capacity(datas.len());

                            for data in datas {
                                fds.push(data.as_raw().fd as i32);
                                offsets.push(data.chunk().offset());
                                strides.push(data.chunk().stride() as u32);
                            }

                            let desktop_size = (
                                user_data.format.size().width,
                                user_data.format.size().height,
                            );
                            let format = user_data.format.format();
                            let modifier = user_data.format.modifier();
                            let mut image = user_data
                                .dma_buf
                                .image_from_dma_buf(
                                    desktop_size,
                                    format,
                                    &fds,
                                    &strides,
                                    &offsets,
                                    modifier,
                                )
                                .unwrap();
                            convert_bgr_to_rgb(&mut image);
                            slint::SharedPixelBuffer::clone_from_slice(
                                &image,
                                user_data.format.size().width,
                                user_data.format.size().height,
                            )
                        } else {
                            // copy frame data to screen
                            let data = &mut datas[0];
                            convert_bgr_to_rgb(data.data().unwrap());
                            slint::SharedPixelBuffer::clone_from_slice(
                                data.data().unwrap(),
                                user_data.format.size().width,
                                user_data.format.size().height,
                            )
                        };

                        frame_sender.send_blocking(buffer).unwrap();
                    }
                }
            })
            .register()?;

        println!("Created stream {:#?}", stream);

        let formats = [
            spa::param::video::VideoFormat::BGRA,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBx,
            spa::param::video::VideoFormat::BGRx,
        ];

        let mut params = Vec::with_capacity(formats.len() * 2);

        for format in formats {
            let modifiers = data
                .borrow()
                .dma_buf
                .query_dma_buf_modifiers(format)
                .unwrap_or(vec![drm::buffer::DrmModifier::Invalid.into()]);
            let modifiers = modifiers.into_iter().map(|m| m as i64).collect::<Vec<_>>();
            let default_modifier = modifiers[0];

            let obj = pw::spa::pod::object!(
                pw::spa::utils::SpaTypes::ObjectParamFormat,
                pw::spa::param::ParamType::EnumFormat,
                pw::spa::pod::property!(
                    pw::spa::param::format::FormatProperties::MediaType,
                    Id,
                    pw::spa::param::format::MediaType::Video
                ),
                pw::spa::pod::property!(
                    pw::spa::param::format::FormatProperties::MediaSubtype,
                    Id,
                    pw::spa::param::format::MediaSubtype::Raw
                ),
                pw::spa::pod::property!(
                    pw::spa::param::format::FormatProperties::VideoFormat,
                    Id,
                    format
                ),
                pw::spa::pod::Property {
                    key: spa::param::format::FormatProperties::VideoModifier.as_raw(),
                    flags: spa::pod::PropertyFlags::MANDATORY
                        | spa::pod::PropertyFlags::DONT_FIXATE,
                    value: spa::pod::Value::Choice(spa::pod::ChoiceValue::Long(
                        spa::utils::Choice(
                            spa::utils::ChoiceFlags::empty(),
                            spa::utils::ChoiceEnum::Enum {
                                default: default_modifier,
                                alternatives: modifiers
                            }
                        )
                    ))
                },
                pw::spa::pod::property!(
                    pw::spa::param::format::FormatProperties::VideoSize,
                    Choice,
                    Range,
                    Rectangle,
                    pw::spa::utils::Rectangle {
                        width: 320,
                        height: 240
                    },
                    pw::spa::utils::Rectangle {
                        width: 1,
                        height: 1
                    },
                    pw::spa::utils::Rectangle {
                        width: 4096,
                        height: 4096
                    }
                ),
                pw::spa::pod::property!(
                    pw::spa::param::format::FormatProperties::VideoFramerate,
                    Choice,
                    Range,
                    Fraction,
                    pw::spa::utils::Fraction { num: 25, denom: 1 },
                    pw::spa::utils::Fraction { num: 0, denom: 1 },
                    pw::spa::utils::Fraction {
                        num: 1000,
                        denom: 1
                    }
                ),
            );
            let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
                std::io::Cursor::new(Vec::new()),
                &pw::spa::pod::Value::Object(obj),
            )
            .unwrap()
            .0
            .into_inner();

            params.push(values);
            // params.push(Pod::from_bytes(&values).unwrap());
        }

        let mut params = params
            .iter()
            .map(|v| spa::pod::Pod::from_bytes(&v).unwrap())
            .collect::<Vec<_>>();

        // for param in &params {
        //     unsafe {
        //         spa_debug_format(2, std::ptr::null(), param.as_raw_ptr());
        //     }
        // }

        stream.connect(
            spa::utils::Direction::Input,
            // None,
            Some(target),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;

        println!("Connected stream, target: {target}");

        Ok(StreamData {
            _stream: stream,
            _stream_listener: stream_listener,
        })
    }

    fn convert_bgr_to_rgb(frame: &mut [u8]) {
        for i in (0..frame.len()).step_by(4) {
            let temp_red = frame[i];
            frame[i] = frame[i + 2];
            frame[i + 2] = temp_red;
        }
    }
}
