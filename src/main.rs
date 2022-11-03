mod render_helpers;
use std::process::exit;

use render_helpers::setup_vulkano;

use crate::render_helpers::{ObjectData, prep_texture, vertex::Vertex};

mod event_loop;
use event_loop::{run_event_loop, Vulkan};

use tobj::LoadOptions;


fn main() {
    println!("Hello, world!");

    //load the OBJ file
    //normally OBJs come with seperate indexes for verts, normals, and UVs
    //this gives us the option of compressing and reordering them in to a single index
    //which is what opengl and vulkan wants
    let load_opts = LoadOptions {
        single_index: true,
        triangulate: true,
        ..Default::default()
    };

    let obj= tobj::load_obj("sphere.obj", &load_opts).unwrap();
    let s = &obj.0[0]; //we only support one model per OBJ file and that is the first object it sees
    
    //all of the code from here forwards is just reorganizing the data in to a Vulkan freindly format
    //I should probably wrap this in a function but whatever
    //read the Obj spec if you actually care about what's happening here, it's really simple, just a lot of loops
    let upper_idx_pos = s.mesh.positions.len();
    let upper_idx_norm = s.mesh.normals.len();
    if upper_idx_pos != upper_idx_norm {
        println!("number of verts doesn't match number of norms???");
        exit(1);
    }

    struct Triple {
        x: f32,
        y: f32,
        z: f32
    }

    let mut coords = Vec::<Triple>::new();
    let mut norms = Vec::<Triple>::new();
    let mut uvs = Vec::<Triple>::new();

    let mut i = 0;
    while i < upper_idx_pos {
        let xp = s.mesh.positions[i];
        let yp = s.mesh.positions[i + 1];
        let zp = s.mesh.positions[i + 2];
        coords.push(Triple { x: xp, y: yp, z: zp});

        let xn = s.mesh.normals[i];
        let yn = s.mesh.normals[i + 1];
        let zn = s.mesh.normals[i + 2];
        norms.push(Triple { x: xn, y: yn, z: zn});

        i += 3;
    }

    let mut j = 0;
    while j < s.mesh.texcoords.len() {
        let u = s.mesh.texcoords[j];
        let v = s.mesh.texcoords[j + 1];
        uvs.push(Triple {x: u, y: v, z: 0.0});
        j += 2;
    }

    let mut verts = Vec::<Vertex>::new();
    for (pos, (norm, uv)) in coords.iter().zip(norms.iter().zip(uvs.iter())) {
        let vert = Vertex {
            position: (pos.x, pos.y, pos.z),
            normal: (norm.x, norm.y, norm.z),
            uv: (uv.x, uv.y)
        };
        verts.push(vert);
    }

    let idxs = s.mesh.indices.to_vec();

    //FIRST BIG OPERATION, THIS SETS UP THE VULKAN CONTEXT
    //SEE render_helpers.rs TO SEE WHATS GOING ON IN HERE
    let vk: Vulkan = setup_vulkano();
    
    //BASE NAME FOR THE TEXTURE WE ARE LOADING
    let base = "MetalPlates006_1K";

    //load each PBR texture, receiving a handle to where the texture is on the GPU
    //and a future that resolves once the texture is on the GPU
    let alb_path = format!("./assets/{}_Color.png", base);
    let alb = prep_texture(&alb_path, vk.queue.clone());

    let rough_path = format!("./assets/{}_Roughness.png", base);
    let rough = prep_texture(&rough_path, vk.queue.clone());

    let metalness_path = format!("./assets/{}_Metalness.png", base);
    let metalness = prep_texture(&metalness_path, vk.queue.clone());

    let normal_path = format!("./assets/{}_Normal.png", base);
    let normal = prep_texture(&normal_path, vk.queue.clone());

    //pack the OBJ data and textures in to a single object that we can pass to the event loop
    let elements = ObjectData {
        vert_data: verts,
        index_data: idxs,
        albedo: alb,
        roughness: rough,
        metallness: metalness,
        normalmap: normal
    };

    //start up the event loop
    run_event_loop(vk, elements);

}
