use std::vec::Vec;

use glium;
use glium::{Display, Surface};

use geometry::polygon::Polygon;
use world::World;


pub struct Ren {
    end_indices: Vec<usize>,
    // OpenGL
    vertex_buffer: glium::VertexBuffer<Vertex>,
    prg: glium::Program,
}

impl Ren {
    pub fn new(display: Display, polygons: &[Polygon]) -> Ren {
        let mut end_indices = Vec::new();
        let mut pos = Vec::new();
        let mut ori = Vec::new();


        let vert_src = include_str!("../../../shaders/xy_tr.vert");
        let frag_src = include_str!("../../../shaders/xy_tr.frag");
        let prg = glium::Program::from_source(&display, vert_src, frag_src, None).unwrap();
        let mut vertices = Vec::new();
        /// / Upload vertices
        for p in polygons {
            for v in &p.points {
                // v: (f32, f32)
                vertices.push(Vertex { pos: [v.0, v.1] });
                debug!["Pushed vertex"; "x" => v.0, "y" => v.1];
            }
            end_indices.push(vertices.len() - 1);
            pos.push(p.pos);
            ori.push(p.ori);
        }
        let vertex_buffer = glium::VertexBuffer::new(&display, &vertices).unwrap();

        Ren {
            end_indices: end_indices,
            vertex_buffer: vertex_buffer,
            prg: prg,
        }

    }

    pub fn render(&self,
                  target: &mut glium::Frame,
                  center: (f32, f32),
                  zoom: f32,
                  width: u32,
                  height: u32,
                  world: &World) {
        let index_buffer = glium::index::NoIndices(glium::index::PrimitiveType::TriangleFan);
        for i in 0..self.end_indices.len() {

            let uniforms = uniform! {
                center: [world.polygons[i].pos.x, world.polygons[i].pos.y],
                orientation: world.polygons[i].ori,
                color: [0.5, 0.5, 0.5],
                proj: super::proj_matrix(width as f32, height as f32, 0.0, 1.0),
                view: super::view_matrix(center.0, center.1, zoom, zoom),
            };

            target.draw(&self.vertex_buffer,
                      &index_buffer,
                      &self.prg,
                      &uniforms,
                      &Default::default())
                .unwrap();
        }
    }
}


#[derive(Copy, Clone)]
struct Vertex {
    pos: [f32; 2],
}

implement_vertex!(Vertex, pos);
