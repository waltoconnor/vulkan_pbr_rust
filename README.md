# Rust Vulkan PBR Render Example
This is a swing I took at implementing a basic Physically Based Rendering shader along with Vulkan rendering using Vulkano. This was my first time doing either of these things, so the code is pretty messy.

You should be able to run this with `cargo run` assuming you have Rust installed and the appropriate Vulkan libraries. What constitutes "Appropriate Vulkan Libraries" is specified in the Vulkano documentation: https://github.com/vulkano-rs/vulkano



I used this renderer as a foundation for showing the different components of the PBR equations in a presentation I gave to an undergraduate computer graphics class so they could use it in their final projects. The slides for that presentation are in `./pbr_slides.pdf` (note that most of the images of things other than the scaley metal sphere are ripped from wikipedia and better written blog posts: https://marmoset.co/posts/physically-based-rendering-and-you-can-too/, and https://learnopengl.com/PBR/Theory).