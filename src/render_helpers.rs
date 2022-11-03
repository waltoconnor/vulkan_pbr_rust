#![allow(dead_code)]
use std::{fs::File, io::{Cursor, Read}, sync::Arc};

use vulkano::{
    command_buffer::{
        CommandBufferExecFuture, 
        PrimaryAutoCommandBuffer, 
        pool::{
            standard::StandardCommandPoolAlloc
        }
    }, 
    device::Queue, 
    format::Format, 
    image::{
        ImageDimensions, 
        ImmutableImage, 
        MipmapsCount, 
        view::ImageView
    }, 
    instance::{
        Instance, 
        PhysicalDevice
    }, 
    render_pass::{
        RenderPass
    }, 
    sync::NowFuture
};
use vulkano::device::{DeviceExtensions, Device};
use vulkano::swapchain::Swapchain;
use vulkano::image::ImageUsage;

use vulkano_win::VkSurfaceBuild;
use winit::{event_loop::EventLoop, window::Window};
use winit::window::WindowBuilder;

#[path = "./vertex.rs"]
pub mod vertex;

//LOADING SHADERS HERE
//kind of strange because shaders are handled at compile time
//so these are imported as modules using macros so that you can typecheck them
//how this works under the hood is a mystery to me
pub mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/pbr_vert.glsl"
    }
}

pub mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/pbr_frag.glsl"
    }
}

//Object to be rendered by the event loop
pub struct ObjectData {
    pub vert_data: Vec<vertex::Vertex>,
    pub index_data: Vec<u32>,
    pub albedo: Texture,
    pub roughness: Texture,
    pub metallness: Texture,
    pub normalmap: Texture
}


//Vulkan context
pub struct Vulkan {
    pub images: Vec<std::sync::Arc<vulkano::image::SwapchainImage<winit::window::Window>>>,
    pub swapchain: Arc<vulkano::swapchain::Swapchain<winit::window::Window>>,
    pub device: Arc<vulkano::device::Device>,
    pub queue: Arc<vulkano::device::Queue>,
    pub events_loop: EventLoop<()>,
    pub surface: Arc<vulkano::swapchain::Surface<winit::window::Window>>,
    pub render_pass: Arc<RenderPass>
}

//Info about a texture, these future types are brutally long
pub struct Texture {
    pub texture: Arc<ImageView<Arc<ImmutableImage>>>, 
    pub tex_future: CommandBufferExecFuture<NowFuture, PrimaryAutoCommandBuffer<StandardCommandPoolAlloc>>
}

//this describes what a render pass looks like
//for us its just Do The Colors -> Update The Depth Buffer
pub fn setup_render_pass(device: Arc<Device>, swapchain: &Swapchain<Window>) -> Arc<RenderPass>{
    let render_pass_noarc = vulkano::single_pass_renderpass!(device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.format(),
                samples: 1,
            },
            depth: {
                load: Clear,
                store: DontCare,
                format: Format::D16Unorm,
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {depth}
        }
    )
    .unwrap();
    let render_pass = Arc::new(render_pass_noarc);

    return render_pass;
}

//Sets up the Vulkano Context
pub fn setup_vulkano() -> Vulkan {
    let required_extension = vulkano_win::required_extensions();
    
    let instance = Instance::new(None, &required_extension, None).unwrap();

    //grab the first device, might be a problem for those with multiple GPUs or and iGPU and dGPU both enabled.
    let physical = PhysicalDevice::enumerate(&instance).next().unwrap();

    println!(
        "Using device: {} (type: {:?})",
        physical.name(),
        physical.ty()
    );
    
    //window event loop
    let event_loop = EventLoop::new();

    //surface to draw on
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    //each graphics card has a number of queues you can submit to
    //this finds a queue that supports drawing to images
    let queue_family = physical.queue_families().find(|&q| {
        println!("found queue: {:?}", q);
        let res = q.supports_graphics() && surface.is_supported(q).unwrap_or(false);
        if res {
            println!("Using this queue");
        }
        return res;
    }).unwrap();

    let device_ext = DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::none()
    };

    //put the infor about the physical device together in to a logical device for Vulkan to interface with
    let (device, mut queues) = Device::new(
        physical,
        physical.supported_features(),
        &device_ext,
        [(queue_family, 0.5)].iter().cloned(),
    ).unwrap();

    //get a queue
    let queue = queues.next().unwrap();

    //set up the initial swapchain configuration
    //the actual stages are fairly self explanatory, but knowing which stages to use is a very advanced topic
    let (swapchain, images) = {
        let caps = surface.capabilities(physical).unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = caps.supported_formats[0].0;
        let dimensions: [u32; 2] = surface.window().inner_size().into();


        Swapchain::start(device.clone(), surface.clone())
            .num_images(caps.min_image_count)
            .format(format)
            .dimensions(dimensions)
            .usage(ImageUsage::color_attachment())
            //.transform(SurfaceTransform::Identity)
            .sharing_mode(&queue)
            .composite_alpha(alpha)
            //.present_mode(PresentMode::Fifo)
            //.fullscreen_exclusive(FullscreenExclusive::Default)
            //.color_space(ColorSpace::SrgbNonLinear)
            .build()
            .unwrap()
    };

    let dev_clone = device.clone();
    let swap_clone = swapchain.clone();

    //return the vulkan context
    Vulkan {
        device: device,
        images: images,
        swapchain: swapchain,
        events_loop: event_loop,
        surface: surface,
        queue: queue,
        render_pass: setup_render_pass(dev_clone, &swap_clone)
    }

    //return (device, images, swapchain, event_loop, surface, queue)
    
}



pub fn prep_texture(path: &String, queue: Arc<Queue>) -> Texture{
    let mut buffer = Vec::new();
    let mut f = File::open(path).unwrap();
    f.read_to_end(&mut buffer).unwrap();
    
    let (texture, tex_future) = {
        //load the PNG
        let png_bytes = buffer;
        let cursor = Cursor::new(png_bytes);
        let decoder = png::Decoder::new(cursor);
        let (info, mut reader) = decoder.read_info().unwrap();
        let dimensions = ImageDimensions::Dim2d {
            width: info.width,
            height: info.height,
            array_layers: 1,
        };

        //each of these stupid PNGs comes in like a billion possible formats
        //we try and handle the most common ones here
        //if this thing guesses wrong it'll screw up how the GPU interprets the bytes of the images
        //Unorm means we are normalizing each value to [0, 1.0] rather than [0, 255]
        //Supported Input Targets: 16 bit grayscale, 8 bit rgb
        let (format, bytes_per_px) = match (info.color_type, info.bit_depth) {
            (png::ColorType::Grayscale, png::BitDepth::Sixteen) => (Format::R16Unorm, 2),
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => (Format::R16G16Unorm, 4),
            (png::ColorType::Grayscale, png::BitDepth::Eight) => (Format::R8Unorm, 1),
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => (Format::R8G8Unorm, 2),
            (png::ColorType::RGB, png::BitDepth::Sixteen) => (Format::R16G16B16Unorm, 6),
            (png::ColorType::RGB, png::BitDepth::Eight) => (Format::R8G8B8Unorm, 3),
            (png::ColorType::RGBA, png::BitDepth::Sixteen) => (Format::R16G16B16A16Unorm,8),
            (png::ColorType::RGBA, png::BitDepth::Eight) => (Format::R8G8B8A8Srgb,4),
            (a, b) => {
                println!("{:?} UNSUPPORTED BIT DEPTH/COLOR TYPE COMBO: {:?}, {:?}, TEXTURE MAY RENDER STRANGELY", path, b, a); 
                (Format::R8G8B8A8Unorm, 4) 
            }
        };

        println!("Selected {:?}, {:?} for {:?}", format, bytes_per_px, path);

        let mut image_data = Vec::new();
        image_data.resize((info.width * info.height * bytes_per_px) as usize, 0);
        reader.next_frame(&mut image_data).unwrap();
        
        //ImmutableImage means that it lives on the GPU and we are not writing to it
        //some dynamic textures will get written to and they need a different thing for this (some games do mirrors like this)
        let (image, future) = ImmutableImage::from_iter(
            image_data.iter().cloned(),
            dimensions,
            MipmapsCount::One,
            format,
            queue.clone(),
        )
        .unwrap();
        (ImageView::new(image).unwrap(), future)
    };

    return Texture { texture: texture, tex_future: tex_future};
}