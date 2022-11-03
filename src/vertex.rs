

#[derive(Default, Debug, Clone)]
pub struct Vertex {
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub uv: (f32, f32),
    
}
vulkano::impl_vertex!(Vertex, position, normal, uv);



