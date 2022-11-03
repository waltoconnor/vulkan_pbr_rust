#[path = "./render_helpers.rs"]
mod render_helpers;
use std::{sync::Arc, time::Instant};
use cgmath::{Matrix4, Point3, Rad, Vector3};
use vulkano::{buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool}, command_buffer::{AutoCommandBufferBuilder, DynamicState, SubpassContents}, descriptor::{PipelineLayoutAbstract, descriptor_set::PersistentDescriptorSet}, device::{Device}, format::Format, image::{AttachmentImage, SwapchainImage, view::ImageView}, pipeline::{GraphicsPipeline, GraphicsPipelineAbstract, vertex::{SingleBufferDefinition}, viewport::Viewport}, render_pass::{Framebuffer, FramebufferAbstract, RenderPass, Subpass}, sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode}, swapchain::{self, AcquireError, SwapchainCreationError}, sync::{self, FlushError, GpuFuture}};
use winit::{event::{Event, WindowEvent}, event_loop::{ControlFlow}, window::Window};

use crate::render_helpers::{ObjectData, vertex::{Vertex}};
use render_helpers::{fs, vs};

pub use crate::render_helpers::Vulkan;

//GETS INVOKED EACH TIME THE WINDOW IS RESIZED,
//NEEDED TO REBUILD THE FRAMEBUFFERS AND PIPELINE WITH THE NEW WINDOW SIZE
fn window_size_dependent_setup(
    device: Arc<Device>,
    images: &[Arc<SwapchainImage<Window>>],
    vs: &vs::Shader,
    fs: &fs::Shader,
    render_pass: Arc<RenderPass>,
) -> (
    Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
) {
    let dimensions = images[0].dimensions();

    let depth_buffer = ImageView::new(
        AttachmentImage::transient(device.clone(), dimensions, Format::D16Unorm).unwrap(),
    )
    .unwrap();

    let framebuffers = images
        .iter()
        .map(|image| {
            let view = ImageView::new(image.clone()).unwrap();
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(view)
                    .unwrap()
                    .add(depth_buffer.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>();

    // In the triangle example we use a dynamic viewport, as its a simple example.
    // However in the teapot example, we recreate the pipelines with a hardcoded viewport instead.
    // This allows the driver to optimize things, at the cost of slower window resizes.
    // https://computergraphics.stackexchange.com/questions/5742/vulkan-best-way-of-updating-pipeline-viewport
    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input(SingleBufferDefinition::<Vertex>::new())
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .viewports(std::iter::once(Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }))
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    );

    (pipeline, framebuffers)
}


//MAIN EVENT LOOP
pub fn run_event_loop(vk: Vulkan, obj_data: ObjectData){

    // let mut dynamic_state = DynamicState {
    //     line_width: None,
    //     viewports: None,
    //     scissors: None,
    //     compare_mask: None,
    //     write_mask: None,
    //     reference: None,
    // };

    let mut swapchain = vk.swapchain;
    
    //This gets set to true either when the swapchain gets filled with garbage and needs to be cleaned up or when the window is resized
    //to be honest I don't understand how the swapchain gets filled with garbage, but that's what the docs say
    let mut recreate_swapchain = false;

    //Load each of the textures on to the GPU asynchronously, *_fut resolves once the GPU has finished loading the texture;
    let alb_fut = obj_data.albedo.tex_future.boxed() as Box<dyn GpuFuture>;
    let met_fut = obj_data.metallness.tex_future.boxed() as Box<dyn GpuFuture>;
    let rou_fut = obj_data.roughness.tex_future.boxed() as Box<dyn GpuFuture>;

    //Join all the futures in to one big future that resolves once all of these are done
    let fut = alb_fut.join(met_fut).join(rou_fut);

    //previous_frame_end is a future that resolves when the GPU is finished displaying the most recently submitted frame
    //We have our first "frame" resolve when the GPU is done loading the textures so that we only start drawing frames once the GPU has all the data it needs
    let mut previous_frame_end = Some(fut.boxed());
    
    //load the shaders
    let vs = vs::Shader::load(vk.device.clone()).unwrap();
    let fs = fs::Shader::load(vk.device.clone()).unwrap();

    //set up the initial GPU pipeline and framebuffers
    //the pipeline describes what steps the GPU should take, for us this is...
    //Load Vertexes -> Apply Vertex Shader -> Setup Viewport (the thing the fragment shader writes to) -> Run Fragment Shader -> Do a Depth Pass -> Render to frame
    //We are running a simple framebuffer setup where we just have a depth buffer and view buffer
    let (mut pipeline, mut framebuffers) = window_size_dependent_setup(vk.device.clone(), &vk.images.to_vec(), &vs, &fs, vk.render_pass.clone());

    //This is the object that describes how we should sample textures
    //This handles mipmapping, what to do with texcoords out of [0.0, 1.0], and how to resolve coordinates that fall between two pixels
    //We use the same sampler for all our textures
    let sampler = Sampler::new(
        vk.device.clone(),
        Filter::Linear,
        Filter::Linear,
        MipmapMode::Nearest,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        0.0,
        1.0,
        0.0,
        0.0,
    )
    .unwrap();

    //This is like the stupid VBO setup in OpenGL except not garbage
    //There are a few buffer types (ImmutableBuffer, CPUAccessibleBuffer, CpuBufferPool), each of which strike a different tradeoff between GPU access speed and CPU access speed
    //CPUAccessibleBuffer is *good enough* for regular GPU rendering, though not as fast as an ImmutableBuffer + some others
    //CpuBufferPool is for data that gets changed *every frame* by the CPU
    let vertex_buffer =
    CpuAccessibleBuffer::from_iter(vk.device.clone(),BufferUsage::all(), false,obj_data.vert_data.iter().cloned())
    .unwrap();

    let idxs_slice = obj_data.index_data.as_slice();
    let index_buffer = CpuAccessibleBuffer::from_iter(vk.device.clone(), BufferUsage::all(), false, idxs_slice.iter().cloned()).unwrap();

    //this is the same as the uniform buffer in OpenGL
    let uniform_buffer = CpuBufferPool::<vs::ty::Data>::new(vk.device.clone(), BufferUsage::all());

    //this is just getting the current clock value
    let rotation_start = Instant::now();

    //we are pulling all these values out of their object because we moved a field out of the vk object earlier so now we cant pass it to the event loop closure since it is partially moved
    //they are fixing this in the next version of rust so that closures have objects whose fields have been moved away as long as they never touch the moved field.
    let surface = vk.surface;
    let device = vk.device;
    let render_pass = vk.render_pass;
    let queue = vk.queue;

    //grab the textures, this is a handle to where the texture is residing on the GPU
    let albedo = obj_data.albedo.texture;
    let roughness = obj_data.roughness.texture;
    let metallness = obj_data.metallness.texture;
    let normalmap = obj_data.normalmap.texture;
    

    //run the main event loop, pass everything in as a closure
    vk.events_loop.run(move |event, _, control_flow| {
        //HANDLE WINDOW EVENTS
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                //kinda cool that you can just modify control flow as a variable, like a weird way of doing exceptions
                *control_flow = ControlFlow::Exit;
            },
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                //when we resize the window we need to remkae the swapchain
                recreate_swapchain = true;
            }
            //THIS IS THE NORMAL ONE THAT RUNS EVERY FRAME
            Event::RedrawEventsCleared => {
                //wait here for the previous frame to finish cleaning up
                previous_frame_end.as_mut().unwrap().cleanup_finished();

                //recompute the dimensions
                let dimensions: [u32; 2] = surface.window().inner_size().into();

                //check if we need to remake the swapchain, if we do, remake it
                if recreate_swapchain {
                    
                    let dimensions: [u32; 2] = surface.window().inner_size().into();
                    let (new_swapchain, new_images) =
                        match swapchain.recreate().dimensions(dimensions).build() {
                            Ok(r) => r,
                            Err(SwapchainCreationError::UnsupportedDimensions) => return,
                            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                        };
                    
                    //update the swapchain, pipeline, and framebuffers
                    swapchain = new_swapchain;
                    let (new_pipeline, new_framebuffers) = window_size_dependent_setup(
                        device.clone(),
                        &new_images,
                        &vs,
                        &fs,
                        render_pass.clone(),
                    );
                    pipeline = new_pipeline;
                    framebuffers = new_framebuffers;
                    recreate_swapchain = false;
                }

                //this is the part of the uniform buffers that gets updated every frame
                //apparently you are supposed to make a different uniform buffer for your frequently and infrequently changed variables
                //but I couldn't figure that out so everything goes in the hot buffer.
                let uniform_buffer_subbuffer = {

                    //this is doing all the same projection math that we did in OpenGL project 2
                    let elapsed = rotation_start.elapsed();
                    let rotation = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1000000000.0;
                    let rotation = Matrix4::from_angle_y(Rad(rotation as f32));
                    
                    let aspect_ratio = dimensions[0] as f32 / dimensions[1] as f32;

                    let proj = cgmath::perspective(Rad(std::f32::consts::FRAC_PI_2), 
                        aspect_ratio, 0.01, 100.0);

                    let camera = Point3::new(1.0, 1.0, 1.0);
                    let look_at = Point3::new(0.0, 0.0, 0.0);
                    let up = Vector3::new(0.0, -1.0, 0.0);
                    let light = Point3::new(0.0, -1.0, -2.0);

                    let view = Matrix4::look_at_rh(camera, look_at, up);
                    let mvp = proj * view * rotation;

                    let lightdir = Vector3::new(look_at.x - light.x, look_at.y - light.y, look_at.z - light.z);

                    //not sure why I need the dummy variables, something strange is going in with the SPIRV compiler
                    //type checker says I need em and it works when I add em, so I'm not going to worry about it
                    let uniform_data = vs::ty::Data {
                        mvp: mvp.into(),
                        camloc: camera.into(),
                        lightdir: lightdir.into(),
                        rotation: rotation.into(),
                        _dummy0: [0,0,0,0],
                        _dummy1: [0,0,0,0],
                    };

                    //jams the data in to the uniform buffer
                    uniform_buffer.next(uniform_data).unwrap()

                };

                //in your shader you have a "set" and "binding" variable. This line picks which set it will be in
                let layout_hot = pipeline.descriptor_set_layout(0).unwrap();

                //this is super inefficient here, should have the textures in a long lived Set (wrapper for uniform vals), and the matricies in a short lived one
                //instead everything goes in the short lived one.
                //THE ORDER THESE ARE ADDED IN CORRESPONDS TO THE binding FEILD IN THE SHADERS
                let set_hot = Arc::new(
                    PersistentDescriptorSet::start(layout_hot.clone())
                        .add_buffer(uniform_buffer_subbuffer)
                        .unwrap()
                        .add_sampled_image(albedo.clone(), sampler.clone())
                        .unwrap()
                        .add_sampled_image(roughness.clone(), sampler.clone())
                        .unwrap()
                        .add_sampled_image(metallness.clone(), sampler.clone())
                        .unwrap()
                        .add_sampled_image(normalmap.clone(), sampler.clone())
                        .unwrap()
                        .build()
                        .unwrap(),
                );

                //not really sure what this is up to, I think it's just trying to get the next frame to draw on from the swapchain
                let (image_num, suboptimal, acquire_future) =
                    match swapchain::acquire_next_image(swapchain.clone(), None) {
                        Ok(r) => r,
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    };
                
                //I think this corresponds to "garbage being in the swapchain"
                if suboptimal {
                    recreate_swapchain = true;
                }

                //earlier we set up a pipeline to describe the order of operations we are doing
                //this describes how to feed stuff in and out of that pipeline
                let mut builder = AutoCommandBufferBuilder::primary(
                    device.clone(),
                    queue.family(),
                    vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit
                )
                .unwrap();

                
                //HERE IS THE ACTUAL OPERATIONS WE ARE RUNNING
                builder
                    .begin_render_pass(
                        framebuffers[image_num].clone(),
                        SubpassContents::Inline,
                        vec![[0.0, 0.0, 1.0, 1.0].into(), 1f32.into()],
                    )
                    .unwrap()
                    .draw_indexed(
                        pipeline.clone(),
                        &DynamicState::none(),
                        vec![vertex_buffer.clone()],
                        index_buffer.clone(),
                        set_hot.clone(),
                        (),
                        vec![],
                    )
                    .unwrap()
                    .end_render_pass()
                    .unwrap();

                let command_buffer = builder.build().unwrap();
                
                //SUBMIT TO THE GPU AND GET A FUTURE BACK
                let future = previous_frame_end
                    .take()
                    .unwrap()
                    .join(acquire_future)
                    .then_execute(queue.clone(), command_buffer)
                    .unwrap()
                    .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
                    .then_signal_fence_and_flush();
                
                //if the future looks good, save it and loop
                //if the future lookds bad, rebuild the swapchain lol
                //if the future looks really bad log an error and try again
                match future {
                    Ok(future) => {
                        previous_frame_end = Some(Box::new(future));
                    },
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        previous_frame_end = Some(Box::new(sync::now(device.clone())))
                    },
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        previous_frame_end = Some(Box::new(sync::now(device.clone())));
                    }
                }
            }
            //this gets called if you hover your mouse over the window so ignore it
            //if you wanted to rotate the camera here is where you would do it
            _ => ()//{ println!("unknown window event: {:?}", event); }
        }
    });
}