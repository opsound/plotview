use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use clap::Parser;
use notify::watcher;
use notify::RecursiveMode;
use notify::Watcher;
use pixels::Pixels;
use pixels::SurfaceTexture;
use tiny_skia::PixmapMut;
use winit::dpi::PhysicalSize;
use winit::event::Event;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    /// Input svg file
    input: Utf8PathBuf,
}

fn read_svg(path: &Utf8Path) -> anyhow::Result<usvg::Tree> {
    println!("reading {}", path);
    let now = Instant::now();
    let svg_data = std::fs::read(path)?;
    let opt = usvg::Options::default();
    let rtree = usvg::Tree::from_data(&svg_data, &opt.to_ref())?;
    println!("took {:?}", now.elapsed());
    Ok(rtree)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("plotview")
        .build(&event_loop)
        .unwrap();

    let mut rtree = read_svg(&args.input)?;

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1))?;
    watcher.watch(&args.input, RecursiveMode::NonRecursive)?;

    let evp = event_loop.create_proxy();
    let input_path = args.input.clone();
    thread::spawn(move || {
        while let Ok(_event) = rx.recv() {
            println!("got file changed event, notifying event loop");
            evp.send_event(()).unwrap();
        }
    });

    println!("allocating buffers");
    let now = Instant::now();
    let PhysicalSize { width, height } = window.inner_size();
    let surface_texture = SurfaceTexture::new(width, height, &window);
    let mut pixbuf = Pixels::new(width, height, surface_texture)?;
    pixbuf.set_clear_color(pixels::wgpu::Color::WHITE);
    println!("took {:?}", now.elapsed());

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(_) => {
                rtree = read_svg(&input_path).unwrap();
                window.request_redraw();
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let PhysicalSize { width, height } = window.inner_size();
                println!("redrawing at {width}x{height}");
                let now = Instant::now();
                pixbuf.resize_surface(width, height);
                pixbuf.resize_buffer(width, height);
                pixbuf.get_frame().fill(0);
                let pixmap = PixmapMut::from_bytes(pixbuf.get_frame(), width, height).unwrap();

                resvg::render(
                    &rtree,
                    usvg::FitTo::Size(width, height),
                    tiny_skia::Transform::default(),
                    pixmap,
                );

                pixbuf.render().unwrap();
                println!("took {:?}", now.elapsed());
            }
            Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
                WindowEvent::Resized(PhysicalSize { width, height }) => {
                    println!("resized to {width}x{height}");
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            _ => (),
        }
    });
}
