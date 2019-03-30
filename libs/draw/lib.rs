#![cfg_attr(
    not(any(
        feature = "vulkan",
        feature = "dx12",
        feature = "metal",
        feature = "gl"
    )),
    allow(dead_code, unused_extern_crates, unused_imports)
)]
#![feature(maybe_uninit)]

extern crate env_logger;
#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;

extern crate glsl_to_spirv;
extern crate image;
extern crate winit;

use hal::format::{AsFormat, ChannelType, Rgba8Srgb as ColorFormat, Swizzle};
use hal::pass::Subpass;
use hal::pso::{PipelineStage, ShaderStageFlags};
use hal::queue::Submission;
use hal::{
    buffer, command, format as f, image as i, memory as m, pass, pool, pso, window::Extent2D,
};
use hal::{Backbuffer, Backend, DescriptorPool, FrameSync, Primitive, SwapchainConfig};
use hal::{Device, Instance, PhysicalDevice, Surface, Swapchain};

use std::fs;
use std::io::{Cursor, Read};

use gfx_hal::command::{CommandBuffer, MultiShot, Primary};

#[cfg_attr(rustfmt, rustfmt_skip)]
const DIMS: Extent2D = Extent2D { width: 1024, height: 768 };

const COLOR_RANGE: i::SubresourceRange = i::SubresourceRange {
    aspects: f::Aspects::COLOR,
    levels: 0..1,
    layers: 0..1,
};

pub struct SwapChainCount {
    current_image: usize,
    image_count: usize,
}

pub trait Canvas {
    fn get_framebuffer(&mut self) -> &mut <back::Backend as Backend>::Framebuffer;
    fn get_queue_group(&mut self) -> &mut hal::QueueGroup<back::Backend, hal::Graphics>;
    fn get_viewport(&mut self) -> &pso::Viewport;
    fn get_swapchain_count(&self) -> SwapChainCount {
        SwapChainCount {
            current_image: 0,
            image_count: 1,
        }
    }
    fn finish(self);
}

pub struct ScreenCanvas<'a, 'b> {
    draw: &'a mut Draw<'b>,
    image_index: u32,
}

impl<'a, 'b> Canvas for ScreenCanvas<'a, 'b> {
    fn get_framebuffer(&mut self) -> &mut <back::Backend as Backend>::Framebuffer {
        &mut self.draw.framebuffers[self.image_index as usize]
    }
    fn get_queue_group(&mut self) -> &mut hal::QueueGroup<back::Backend, hal::Graphics> {
        &mut self.draw.queue_group
    }
    fn get_viewport(&mut self) -> &pso::Viewport {
        &self.draw.viewport
    }
    fn finish(self) {
        self.draw.swap_it(self.image_index);
    }
}

impl<'a, 'b> ScreenCanvas<'a, 'b> {
    fn do_swap(&mut self) {
        let mut cmd_buffer = self
            .draw
            .command_pool
            .acquire_command_buffer::<command::OneShot>();
        unsafe {
            cmd_buffer.begin();
            cmd_buffer.finish();
            let index = self.draw.frame_index;
            self.draw.queue_group.queues[0].submit_nosemaphores(
                std::iter::once(&cmd_buffer),
                Some(&self.draw.frame_fence[index]),
            );
        }
        self.draw.swap_it(self.image_index);
    }
}

impl<'a, 'b> Drop for ScreenCanvas<'a, 'b> {
    fn drop(&mut self) {
        self.do_swap();
    }
}

pub struct DynamicBinaryTexture<'a> {
    // buffer: <back::Backend as Backend>::Buffer,
    // buffer_size: u64,
    // cmd_buffer: CommandBuffer<back::Backend, hal::Graphics, command::OneShot, Primary>,
    // desc_set: <back::Backend as Backend>::DescriptorSet,
    device: &'a back::Device,
    // image_upload_buffer: <back::Backend as Backend>::Buffer,
    // instance_buffer: <back::Backend as Backend>::Buffer,
    // instance_buffer_memory: <back::Backend as Backend>::Memory,
    // instance_count: u32,
    // memory: <back::Backend as Backend>::Memory,
    // memory_fence: <back::Backend as Backend>::Fence,
    // pipeline: <back::Backend as Backend>::GraphicsPipeline,
    // pipeline_layout: <back::Backend as Backend>::PipelineLayout,
    // render_pass: <back::Backend as Backend>::RenderPass,
}

pub struct Bullets<'a> {
    buffer: <back::Backend as Backend>::Buffer,
    buffer_size: u64,
    cmd_buffer: CommandBuffer<back::Backend, hal::Graphics, command::OneShot, Primary>,
    desc_set: <back::Backend as Backend>::DescriptorSet,
    device: &'a back::Device,
    image_upload_buffer: <back::Backend as Backend>::Buffer,
    instance_buffer: <back::Backend as Backend>::Buffer,
    instance_buffer_memory: <back::Backend as Backend>::Memory,
    instance_count: u32,
    memory: <back::Backend as Backend>::Memory,
    memory_fence: <back::Backend as Backend>::Fence,
    pipeline: <back::Backend as Backend>::GraphicsPipeline,
    pipeline_layout: <back::Backend as Backend>::PipelineLayout,
    render_pass: <back::Backend as Backend>::RenderPass,
}
impl<'a> Bullets<'a> {
    pub fn upload(&mut self, data: &[f32]) {
        unsafe {
            self.device
                .wait_for_fence(&self.memory_fence, u64::max_value());
        }
        unsafe {
            // const QUAD: [f32; 6] = [0.2, 0.3, 0.0, -0.1, -0.3, 0.5];
            println!["{:?}", self.buffer_size];
            let mut vertices = self
                .device
                .acquire_mapping_writer::<f32>(&self.instance_buffer_memory, 0..self.buffer_size)
                .unwrap();
            vertices[0..data.len()].copy_from_slice(data);
            self.device.release_mapping_writer(vertices).unwrap();
        }
        assert![data.len() % 3 == 0];
        self.instance_count = (data.len() / 3) as u32;
    }
    pub fn draw(&mut self, surface: &mut impl Canvas) {
        unsafe {
            self.cmd_buffer.begin();

            self.cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            self.cmd_buffer.bind_vertex_buffers(
                0,
                [(&self.buffer, 0u64), (&self.instance_buffer, 0u64)]
                    .iter()
                    .cloned(),
            );
            self.cmd_buffer.bind_graphics_descriptor_sets(
                &self.pipeline_layout,
                0,
                Some(&self.desc_set),
                &[],
            );

            {
                let rect = surface.get_viewport().rect.clone();
                let mut encoder = self.cmd_buffer.begin_render_pass_inline(
                    &self.render_pass,
                    surface.get_framebuffer(),
                    rect,
                    &[],
                );
                encoder.draw(0..6, 0..self.instance_count);
            }

            self.cmd_buffer.finish();

            self.device.reset_fence(&self.memory_fence);
            surface.get_queue_group().queues[0]
                .submit_nosemaphores(std::iter::once(&self.cmd_buffer), Some(&self.memory_fence));
        }
    }
}

impl<'a> Drop for Bullets<'a> {
    fn drop(&mut self) {
        unsafe {
            // self.device.wait_for_fence(&self.memory_fence, u64::max_value());

            // let buffer = std::mem::replace(&mut self.buffer, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_buffer(buffer);

            // // No cmd_buffer free?

            // let image_upload_buffer = std::mem::replace(&mut self.image_upload_buffer, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_buffer(image_upload_buffer);

            // let memory = std::mem::replace(&mut self.memory, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.free_memory(memory);

            // let memory_fence = std::mem::replace(&mut self.memory_fence, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_fence(memory_fence);

            // let pipeline = std::mem::replace(&mut self.pipeline, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_graphics_pipeline(pipeline);

            // let render_pass = std::mem::replace(&mut self.render_pass, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_render_pass(render_pass);
        }
    }
}
pub struct StaticTexture2DRectangle<'a> {
    buffer: <back::Backend as Backend>::Buffer,
    cmd_buffer: CommandBuffer<back::Backend, hal::Graphics, command::OneShot, Primary>,
    device: &'a back::Device,
    image_upload_buffer: <back::Backend as Backend>::Buffer,
    memory: <back::Backend as Backend>::Memory,
    memory_fence: <back::Backend as Backend>::Fence,
    pipeline: <back::Backend as Backend>::GraphicsPipeline,
    render_pass: <back::Backend as Backend>::RenderPass,
}
impl<'a> StaticTexture2DRectangle<'a> {
    pub fn draw(&mut self, surface: &mut impl Canvas) {
        unsafe {
            self.cmd_buffer.begin();

            // let mut x = draw.viewport.clone();
            // self.cmd_buffer.set_viewports(0, &[x]);
            // self.cmd_buffer.set_scissors(0, &[draw.viewport.rect]);
            self.cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            self.cmd_buffer
                .bind_vertex_buffers(0, Some((&self.buffer, 0)));
            // cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout, 0, Some(&self.desc_set), &[]);

            {
                let rect = surface.get_viewport().rect.clone();
                let mut encoder = self.cmd_buffer.begin_render_pass_inline(
                    &self.render_pass,
                    surface.get_framebuffer(),
                    rect,
                    &[],
                );
                encoder.draw(0..6, 0..1);
            }

            self.cmd_buffer.finish();

            surface.get_queue_group().queues[0]
                .submit_nosemaphores(std::iter::once(&self.cmd_buffer), None);
        }
    }
}

impl<'a> Drop for StaticTexture2DRectangle<'a> {
    fn drop(&mut self) {
        unsafe {
            // self.device.wait_for_fence(&self.memory_fence, u64::max_value());

            // let buffer = std::mem::replace(&mut self.buffer, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_buffer(buffer);

            // // No cmd_buffer free?

            // let image_upload_buffer = std::mem::replace(&mut self.image_upload_buffer, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_buffer(image_upload_buffer);

            // let memory = std::mem::replace(&mut self.memory, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.free_memory(memory);

            // let memory_fence = std::mem::replace(&mut self.memory_fence, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_fence(memory_fence);

            // let pipeline = std::mem::replace(&mut self.pipeline, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_graphics_pipeline(pipeline);

            // let render_pass = std::mem::replace(&mut self.render_pass, std::mem::MaybeUninit::uninitialized().into_inner());
            // self.device.destroy_render_pass(render_pass);
        }
    }
}

pub struct StaticWhite2DTriangle {
    buffer: <back::Backend as Backend>::Buffer,
    cmd_buffer: CommandBuffer<back::Backend, hal::Graphics, MultiShot, Primary>,
    memory: <back::Backend as Backend>::Memory,
    memory_fence: <back::Backend as Backend>::Fence,
    pipeline: <back::Backend as Backend>::GraphicsPipeline,
    render_pass: <back::Backend as Backend>::RenderPass,
}

impl StaticWhite2DTriangle {
    pub fn draw(&mut self, surface: &mut impl Canvas) {
        unsafe {
            self.cmd_buffer.begin(false);

            // let mut x = draw.viewport.clone();
            // self.cmd_buffer.set_viewports(0, &[x]);
            // self.cmd_buffer.set_scissors(0, &[draw.viewport.rect]);
            self.cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            self.cmd_buffer
                .bind_vertex_buffers(0, Some((&self.buffer, 0)));
            // cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout, 0, Some(&self.desc_set), &[]);

            {
                let rect = surface.get_viewport().rect.clone();
                let mut encoder = self.cmd_buffer.begin_render_pass_inline(
                    &self.render_pass,
                    surface.get_framebuffer(),
                    rect,
                    &[],
                );
                encoder.draw(0..3, 0..1);
            }

            self.cmd_buffer.finish();

            surface.get_queue_group().queues[0]
                .submit_nosemaphores(std::iter::once(&self.cmd_buffer), None);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub points: [[f32; 2]; 3],
}

impl Triangle {
    pub fn points_flat(self) -> [f32; 6] {
        let [[a, b], [c, d], [e, f]] = self.points;
        [a, b, c, d, e, f]
    }
}

pub struct Draw<'a> {
    adapter: hal::Adapter<back::Backend>,
    command_pool: hal::CommandPool<back::Backend, hal::Graphics>,
    device: &'a back::Device,
    format: hal::format::Format,
    frame_fence: Vec<<back::Backend as Backend>::Fence>,
    frame_index: usize,
    frame_semaphore: Vec<<back::Backend as Backend>::Semaphore>,
    framebuffers: Vec<<back::Backend as Backend>::Framebuffer>,
    image_count: usize,
    queue_group: hal::QueueGroup<back::Backend, hal::Graphics>,
    render_finished_semaphore: Vec<<back::Backend as Backend>::Semaphore>,
    swap_chain: <back::Backend as Backend>::Swapchain,
    viewport: pso::Viewport,
}

struct Y<'a, 'b> {
    data: &'b mut X<'a>,
}
impl<'a, 'b> Y<'a, 'b> {
    fn yeet(&mut self) {}
}
struct X<'a> {
    a: &'a mut i32,
}
impl<'a> X<'a> {
    fn dox<'b>(&'b mut self) -> Y<'b, 'a> {
        Y { data: self }
    }
}

fn abba() {
    let mut a = 123;
    // let mut eks = X { a: &mut a };
    // let mut k = eks.dox();
    // let mut m = eks.dox();
    // k.yeet(); // illegal
    // m.yeet(); // nice
}

impl<'a> Draw<'a> {
    pub fn prepare_canvas<'b>(&'b mut self) -> ScreenCanvas<'b, 'a> {
        let image = self.acquire_swapchain_image().unwrap();
        self.clear(image, 0.3);
        ScreenCanvas {
            draw: self,
            image_index: image,
        }
    }

    pub fn open_device(
        surface: &mut <back::Backend as Backend>::Surface,
        adapters: &mut Vec<hal::Adapter<back::Backend>>,
    ) -> (
        back::Device,
        hal::QueueGroup<back::Backend, hal::Graphics>,
        hal::Adapter<back::Backend>,
    ) {
        // Step 1: Find devices on machine
        for adapter in adapters.iter() {
            println!("Adapter: {:?}", adapter.info);
        }
        let mut adapter = adapters.remove(0);
        // let memory_types = adapter.physical_device.memory_properties().memory_types;
        // let limits = adapter.physical_device.limits();
        // Step 2: Open device supporting Graphics
        let (device, queue_group) = adapter
            .open_with::<_, hal::Graphics>(1, |family| surface.supports_queue_family(family))
            .expect("Unable to find device supporting graphics");
        (device, queue_group, adapter)
    }

    pub fn new<'b: 'a>(
        surface: &mut <back::Backend as Backend>::Surface,
        device: &'b back::Device,
        queue_group: hal::QueueGroup<back::Backend, hal::Graphics>,
        mut adapter: hal::Adapter<back::Backend>,
    ) -> Self {
        // Step 3: Create command pool
        let command_pool = unsafe {
            device.create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty())
        }
        .expect("Can't create command pool");
        // Step 4: Set up swapchain
        let (caps, formats, present_modes) = surface.compatibility(&mut adapter.physical_device);
        let format = formats.map_or(f::Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });
        let present_mode = {
            use gfx_hal::window::PresentMode::*;
            [Mailbox, Fifo, Relaxed, Immediate]
                .iter()
                .cloned()
                .find(|pm| present_modes.contains(pm))
                .ok_or("No PresentMode values specified!")
                .unwrap()
        };
        println!["{:?}", present_modes];
        println!["{:?}", present_mode];
        println!["{:?}", caps];

        use gfx_hal::window::PresentMode::*;
        let image_count = if present_mode == Mailbox {
            (caps.image_count.end - 1).min(3) as usize
        } else {
            (caps.image_count.end - 1).min(2) as usize
        };

        let swap_config = SwapchainConfig::from_caps(&caps, format, DIMS);
        println!("{:?}", swap_config);
        let extent = swap_config.extent.to_extent();

        let (swap_chain, backbuffer) =
            unsafe { device.create_swapchain(surface, swap_config, None) }
                .expect("Can't create swapchain");
        // Step 5: Create render pass
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Load,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: i::Access::empty()
                    ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            };

            unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
                .expect("Can't create render pass")
        };
        // Step 6: Collect framebuffers
        let (frame_images, framebuffers) = match backbuffer {
            Backbuffer::Images(images) => {
                println!["Image backbuffer"];
                let pairs = images
                    .into_iter()
                    .map(|image| unsafe {
                        let rtv = device
                            .create_image_view(
                                &image,
                                i::ViewKind::D2,
                                format,
                                Swizzle::NO,
                                COLOR_RANGE.clone(),
                            )
                            .unwrap();
                        (image, rtv)
                    })
                    .collect::<Vec<_>>();
                let fbos = pairs
                    .iter()
                    .map(|&(_, ref rtv)| unsafe {
                        device
                            .create_framebuffer(&render_pass, Some(rtv), extent)
                            .unwrap()
                    })
                    .collect();
                (pairs, fbos)
            }
            Backbuffer::Framebuffer(fbo) => {
                println!["Framebuffer backbuffer"];
                (Vec::new(), vec![fbo])
            }
        };

        // Step 7: Set up a viewport
        let viewport = pso::Viewport {
            rect: pso::Rect {
                x: 0,
                y: 0,
                w: extent.width as _,
                h: extent.height as _,
            },
            depth: 0.0..1.0,
        };

        // Step 8: Set up fences and semaphores
        let mut frame_fence = Vec::with_capacity(image_count);
        let mut frame_semaphore = Vec::with_capacity(image_count);
        let mut render_finished_semaphore = Vec::with_capacity(image_count);
        for i in 0..image_count {
            frame_fence.push(device.create_fence(true).expect("Can't create fence"));
            frame_semaphore.push(device.create_semaphore().expect("Can't create semaphore"));
            render_finished_semaphore
                .push(device.create_semaphore().expect("Can't create semaphore"));
        }

        Self {
            adapter,
            command_pool,
            device,
            format,
            frame_fence,
            frame_index: 0,
            frame_semaphore,
            framebuffers,
            image_count,
            queue_group,
            render_finished_semaphore,
            swap_chain,
            viewport,
        }
    }

    fn acquire_swapchain_image(&mut self) -> Option<hal::SwapImageIndex> {
        unsafe {
            // self.command_pool.reset();
            match self.swap_chain.acquire_image(
                u64::max_value(),
                FrameSync::Semaphore(&mut self.frame_semaphore[self.frame_index]),
            ) {
                Ok(i) => {
                    self.frame_index = (self.frame_index + 1) % self.image_count;
                    self.device
                        .reset_fence(&self.frame_fence[self.frame_index])
                        .unwrap();
                    Some(i)
                }
                Err(_) => None,
            }
        }
    }
    pub fn swap_it(&mut self, frame: hal::SwapImageIndex) {
        unsafe {
            self.device
                .wait_for_fence(&self.frame_fence[self.frame_index], u64::max_value());
            if let Err(_) = self
                .swap_chain
                .present_nosemaphores(&mut self.queue_group.queues[0], frame)
            {
                // self.recreate_swapchain = true;
            }
        }
    }

    pub fn create_dynamic_binary_texture<'b>(
        &mut self,
        device: &'b back::Device,
        rows: usize,
        image: &[u8],
    ) -> DynamicBinaryTexture<'b> {
        static VERTEX_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable
        layout(location = 0) in vec2 pos;
        layout(location = 0) out vec2 texpos;

        out gl_PerVertex {
            vec4 gl_Position;
        };

        void main() {
            texpos = (pos + 1)/2;
            gl_Position = vec4(pos, 0, 1);
        }";
        static FRAGMENT_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable
        layout(location = 0) in vec2 texpos;
        layout(location = 0) out vec4 Color;

        layout(constant_id = 0) const float rand_seed1 = 0.0f;
        layout(constant_id = 1) const float rand_seed2 = 0.0f;
        layout(constant_id = 2) const float rand_seed3 = 0.0f;
        layout(constant_id = 3) const float width = 1.2f;

        // Hash function: http://amindforeverprogramming.blogspot.com/2013/07/random-floats-in-glsl-330.html
        uint hash( uint x ) {
            x += ( x << 10u );
            x ^= ( x >>  6u );
            x += ( x <<  3u );
            x ^= ( x >> 11u );
            x += ( x << 15u );
            return x;
        }
        uint hash(uvec3 v) {
            return hash( v.x ^ hash(v.y) ^ hash(v.z) );
        }
        float random(uvec3 pos) {
            const uint mantissaMask = 0x007FFFFFu;
            const uint one          = 0x3F800000u;

            uint h = hash( pos );
            h &= mantissaMask;
            h |= one;

            float  r2 = uintBitsToFloat( h );
            return r2 - 1.0;
        }
        float random(vec3 pos) {
            return random(floatBitsToUint(pos));
        }
        // returns fraction part
        float separate(float n, out float i) {
            float frac = modf(n, i);
            if (n < 0.f) {
                frac = 1 + frac; // make fraction non-negative and invert (1 - frac)
                i --;
            }
            return frac;
        }

        // Perlin: http://www.iquilezles.org/www/articles/morenoise/morenoise.htm
        float perlin(vec3 pos, out float dnx, out float dny, out float dnz) {
            float i, j, k;
            float u, v, w;

            // Separate integer and fractional part of coordinates
            u = separate( pos.x, i);
            v = separate( pos.y, j);
            w = separate( pos.z, k);


            float du = 30.0f*u*u*(u*(u-2.0f)+1.0f);
            float dv = 30.0f*v*v*(v*(v-2.0f)+1.0f);
            float dw = 30.0f*w*w*(w*(w-2.0f)+1.0f);

            u = u*u*u*(u*(u*6.0f-15.0f)+10.0f);
            v = v*v*v*(v*(v*6.0f-15.0f)+10.0f);
            w = w*w*w*(w*(w*6.0f-15.0f)+10.0f);

            float a = random( vec3(i+0, j+0, k+0) );
            float b = random( vec3(i+1, j+0, k+0) );
            float c = random( vec3(i+0, j+1, k+0) );
            float d = random( vec3(i+1, j+1, k+0) );
            float e = random( vec3(i+0, j+0, k+1) );
            float f = random( vec3(i+1, j+0, k+1) );
            float g = random( vec3(i+0, j+1, k+1) );
            float h = random( vec3(i+1, j+1, k+1) );

            float k0 =   a;
            float k1 =   b - a;
            float k2 =   c - a;
            float k3 =   e - a;
            float k4 =   a - b - c + d;
            float k5 =   a - c - e + g;
            float k6 =   a - b - e + f;
            float k7 = - a + b + c - d + e - f - g + h;

            /* dnx = du * (k1 + k4*v + k6*w + k7*v*w); */
            /* dny = dv * (k2 + k5*w + k4*u + k7*w*u); */
            /* dnz = dw * (k3 + k6*u + k5*v + k7*u*v); */
            return k0 + k1*u + k2*v + k3*w + k4*u*v + k5*v*w + k6*w*u + k7*u*v*w;
        }

        // Note: It starts (octave 1) with the highest frequency, `width`
        float FBM(vec3 pos, int octaves) {
            float a, b, c;
            float result = 0;
            float p;

            pos *= width; // Frequency = pixel
            /* pos *= 1000; */

            const float power = 3;  // Higher -> lower frequencies dominate. Normally 2.
            float pos_factor = 1.f;
            float strength_factor = 1.f / pow(power, octaves);
            for (int i = 0; i < octaves; i ++)
            {
                p = perlin(pos * pos_factor, a, b, c );
                result += (power - 1) * strength_factor * p;

                pos_factor *= 0.5f;
                strength_factor *= power;
            }

            return result;
        }

        void main()
        {
            int octaves = 8;
            float r;
            r = FBM(vec3(texpos,0) + vec3(rand_seed1, rand_seed2, rand_seed3), octaves);
            r = step(0.5, r);
            Color = vec4(vec3(r), 1);
        }";
        static ENTRY_NAME: &str = "main";
        let vs_module = {
            let glsl = VERTEX_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Vertex)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        let fs_module = {
            let glsl = FRAGMENT_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Fragment)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        // Create a render pass for this thing
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(self.format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            // let dependency = pass::SubpassDependency {
            //     passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
            //     stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
            //         ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            //     accesses: i::Access::empty()
            //         ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            // };

            unsafe { device.create_render_pass(&[attachment], &[subpass], &[]) }
                .expect("Can't create render pass")
        };
        let kind = i::Kind::D2(1000 as i::Size, 1000 as i::Size, 1, 1);
        let mut image_logo = unsafe {
            device.create_image(
                kind,
                1,
                // ColorFormat::SELF,
                hal::format::Format::Rgba8Srgb,
                i::Tiling::Linear,
                i::Usage::TRANSFER_DST | i::Usage::SAMPLED,
                i::ViewCapabilities::empty(),
            )
        }
        .unwrap();
        let image_req = unsafe { device.get_image_requirements(&image_logo) };
        use gfx_hal::{adapter::MemoryTypeId, memory::Properties};
        let device_type = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                image_req.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();
        let image_memory = unsafe { device.allocate_memory(device_type, image_req.size) }.unwrap();
        println!["image req image n42cp {:?}", image_req];
        unsafe { device.bind_image_memory(&image_memory, 0, &mut image_logo) }.unwrap();
        let image_srv = unsafe {
            device.create_image_view(
                &image_logo,
                i::ViewKind::D2,
                ColorFormat::SELF,
                Swizzle::NO,
                COLOR_RANGE.clone(),
            )
        }
        .unwrap();
        let extent = i::Extent {
            width: 1000,
            height: 1000,
            depth: 1,
        };
        let fbo = unsafe {
            device
                .create_framebuffer(&render_pass, Some(image_srv), extent)
                .unwrap()
        };
        let (vs_entry, fs_entry) = (
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &vs_module,
                specialization: pso::Specialization {
                    constants: &[
                        pso::SpecializationConstant { id: 0, range: 0..1 },
                        pso::SpecializationConstant { id: 1, range: 0..1 },
                        pso::SpecializationConstant { id: 2, range: 0..1 },
                        pso::SpecializationConstant { id: 3, range: 0..1 },
                    ],
                    data: unsafe {
                        std::mem::transmute::<&[f32; 4], &[u8; 16]>(&[
                            0.8f32, 0.3f32, 0.1f32, 3912.0f32,
                        ])
                    },
                },
            },
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &fs_module,
                specialization: pso::Specialization::default(),
            },
        );
        println!["Making shader set"];
        let shader_entries = pso::GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };
        // let set_layout = unsafe {
        //     device.create_descriptor_set_layout(
        //         &[
        //             pso::DescriptorSetLayoutBinding {
        //                 binding: 0,
        //                 ty: pso::DescriptorType::SampledImage,
        //                 count: 1,
        //                 stage_flags: ShaderStageFlags::FRAGMENT,
        //                 immutable_samplers: false,
        //             },
        //             pso::DescriptorSetLayoutBinding {
        //                 binding: 1,
        //                 ty: pso::DescriptorType::Sampler,
        //                 count: 1,
        //                 stage_flags: ShaderStageFlags::FRAGMENT,
        //                 immutable_samplers: false,
        //             },
        //         ],
        //         &[],
        //     )
        // }
        // .expect("Can't create descriptor set layout");
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                // std::iter::once(&set_layout),
                &[], // No descriptor set layout (no texture/sampler)
                &[(pso::ShaderStageFlags::VERTEX, 0..8)],
            )
        }
        .expect("Cant create pipelinelayout");
        let subpass = Subpass {
            index: 0,
            main_pass: &render_pass,
        };
        let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
            shader_entries,
            Primitive::TriangleList,
            pso::Rasterizer::FILL,
            &pipeline_layout,
            subpass,
        );

        pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
            binding: 0,
            stride: 8 as u32,
            rate: pso::VertexInputRate::Vertex,
            // 0 = Per Vertex
            // 1 = Per Instance
        });
        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 0,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        });
        pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
            pso::ColorMask::ALL,
            pso::BlendState::ALPHA,
        ));
        let pipeline = unsafe {
            device
                .create_graphics_pipeline(&pipeline_desc, None)
                .expect("Unable to make")
        };
        let mut vertex_buffer =
            unsafe { device.create_buffer(4 * 6 * 4, buffer::Usage::VERTEX) }.unwrap();
        let requirements = unsafe { device.get_buffer_requirements(&vertex_buffer) };
        let memory_type_id = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();
        let buffer_memory =
            unsafe { device.allocate_memory(memory_type_id, requirements.size) }.unwrap();
        unsafe { device.bind_buffer_memory(&buffer_memory, 0, &mut vertex_buffer) }.unwrap();
        unsafe {
            const QUAD: [f32; 4 * 6] = [
                -0.5, 0.33, 0.0, 1.0, 0.5, 0.33, 1.0, 1.0, 0.5, -0.33, 1.0, 0.0, -0.5, 0.33, 0.0,
                1.0, 0.5, -0.33, 1.0, 0.0, -0.5, -0.33, 0.0, 0.0,
            ];
            let mut vertices = device
                .acquire_mapping_writer::<f32>(&buffer_memory, 0..requirements.size)
                .unwrap();
            vertices[0..QUAD.len()].copy_from_slice(&QUAD);
            device.release_mapping_writer(vertices).unwrap();
        }
        // Section 2, draw it
        unsafe {
            let mut cmd_buffer = self
                .command_pool
                .acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();
            // Unfortunately not in GL
            // cmd_buffer.push_graphics_constants(&pipeline_layout, pso::ShaderStageFlags::FRAGMENT, 0, &[1, 2, 3, 4]);
            cmd_buffer.bind_graphics_pipeline(&pipeline);
            cmd_buffer.bind_vertex_buffers(0, [(&vertex_buffer, 0u64)].iter().cloned());
            {
                let mut pass = cmd_buffer.begin_render_pass_inline(
                    &render_pass,
                    &fbo,
                    pso::Rect {
                        x: 0,
                        y: 0,
                        w: 1000,
                        h: 1000,
                    },
                    &[command::ClearValue::Color(command::ClearColor::Float([
                        0.0, 0.0, 0.0, 1.0,
                    ]))],
                );
                pass.draw(0..6, 0..1);
            }
            cmd_buffer.finish();
            let fence = device.create_fence(false).unwrap();
            self.queue_group.queues[0].submit_nosemaphores(Some(&cmd_buffer), Some(&fence));
            println!["waiting for fence"];
            device.wait_for_fence(&fence, u64::max_value()).unwrap();
            println!["fence released"];
            device.destroy_fence(fence);
            println!["fence destroyed"];
        };

        unsafe { device.bind_image_memory(&image_memory, 0, &mut image_logo) }.unwrap();
        unsafe {
            let reader = device
                .acquire_mapping_reader::<f32>(&image_memory, 0..image_req.size)
                .unwrap();
            device.release_mapping_reader(reader);
        };
        println!["exint"];
        DynamicBinaryTexture { device }
    }

    pub fn create_bullets<'b>(&mut self, device: &'b back::Device, image: &[u8]) -> Bullets<'b> {
        const VERTEX_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable

        layout(constant_id = 0) const float scale = 1.2f;

        layout(location = 0) in vec2 a_pos;
        layout(location = 1) in vec2 a_uv;
        layout(location = 2) in vec2 a_move;
        layout(location = 3) in float a_rot;
        layout(location = 0) out vec2 v_uv;

        out gl_PerVertex {
            vec4 gl_Position;
        };

        void main() {
            v_uv = a_uv;
            float r = a_rot;
            gl_Position = mat4(
                cos(r), -sin(r), 0, 0,
                sin(r),  cos(r), 0, 0,
                0,       0,      1, 0,
                0,       0,      0, 1) * vec4(scale * a_pos, 0.0, 1.0) + vec4(a_move, 0, 0);
        }";

        const FRAGMENT_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable

        layout(location = 0) in vec2 v_uv;
        layout(location = 0) out vec4 target0;

        layout(set = 0, binding = 0) uniform texture2D u_texture;
        layout(set = 0, binding = 1) uniform sampler u_sampler;

        void main() {
            target0 = texture(sampler2D(u_texture, u_sampler), v_uv);
        }";
        let set_layout = unsafe {
            device.create_descriptor_set_layout(
                &[
                    pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::SampledImage,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    pso::DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: pso::DescriptorType::Sampler,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
                &[],
            )
        }
        .expect("Can't create descriptor set layout");

        // Descriptors
        let mut desc_pool = unsafe {
            device.create_descriptor_pool(
                1, // sets
                &[
                    pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::SampledImage,
                        count: 1,
                    },
                    pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Sampler,
                        count: 1,
                    },
                ],
                pso::DescriptorPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create descriptor pool");
        let desc_set = unsafe { desc_pool.allocate_set(&set_layout) }.unwrap();

        // Allocate memory for Vertices and UV
        const F32_SIZE: u64 = std::mem::size_of::<f32>() as u64;
        const F32_PER_VERTEX: u64 = 2 + 2; // (x, y, u, v)
        const VERTICES: u64 = 6; // Using a triangle fan, which is the most optimal
        let mut vertex_buffer = unsafe {
            device.create_buffer(F32_SIZE * F32_PER_VERTEX * VERTICES, buffer::Usage::VERTEX)
        }
        .unwrap();
        let requirements = unsafe { device.get_buffer_requirements(&vertex_buffer) };

        use gfx_hal::{adapter::MemoryTypeId, memory::Properties};
        let memory_type_id = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();

        let buffer_memory =
            unsafe { device.allocate_memory(memory_type_id, requirements.size) }.unwrap();
        unsafe { device.bind_buffer_memory(&buffer_memory, 0, &mut vertex_buffer) }.unwrap();
        unsafe {
            const QUAD: [f32; (F32_PER_VERTEX * VERTICES) as usize] = [
                -0.5, 0.33, 0.0, 1.0, 0.5, 0.33, 1.0, 1.0, 0.5, -0.33, 1.0, 0.0, -0.5, 0.33, 0.0,
                1.0, 0.5, -0.33, 1.0, 0.0, -0.5, -0.33, 0.0, 0.0,
            ];
            let mut vertices = device
                .acquire_mapping_writer::<f32>(&buffer_memory, 0..requirements.size)
                .unwrap();
            vertices[0..QUAD.len()].copy_from_slice(&QUAD);
            device.release_mapping_writer(vertices).unwrap();
        }

        let mut instance_buffer =
            unsafe { device.create_buffer(1000000, buffer::Usage::VERTEX) }.unwrap();
        let instance_buffer_requirements =
            unsafe { device.get_buffer_requirements(&instance_buffer) };

        let instance_buffer_memory_type_id = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                instance_buffer_requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();

        let instance_buffer_memory = unsafe {
            device.allocate_memory(
                instance_buffer_memory_type_id,
                instance_buffer_requirements.size,
            )
        }
        .unwrap();
        unsafe { device.bind_buffer_memory(&instance_buffer_memory, 0, &mut instance_buffer) }
            .unwrap();
        unsafe {
            const QUAD: [f32; 6] = [0.2, 0.3, 0.0, -0.1, -0.3, 0.5];
            let mut vertices = device
                .acquire_mapping_writer::<f32>(&instance_buffer_memory, 0..requirements.size)
                .unwrap();
            vertices[0..QUAD.len()].copy_from_slice(&QUAD);
            device.release_mapping_writer(vertices).unwrap();
        }

        let img_data = image;
        let img = image::load(Cursor::new(&img_data[..]), image::PNG)
            .unwrap()
            .to_rgba();
        let (width, height) = img.dimensions();
        let kind = i::Kind::D2(width as i::Size, height as i::Size, 1, 1);
        let limits = self.adapter.physical_device.limits();
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;
        let image_stride = 4usize;
        let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
        let upload_size = (height * row_pitch) as u64;

        let mut image_upload_buffer =
            unsafe { device.create_buffer(upload_size, buffer::Usage::TRANSFER_SRC) }.unwrap();
        let image_mem_reqs = unsafe { device.get_buffer_requirements(&image_upload_buffer) };
        let image_upload_memory =
            unsafe { device.allocate_memory(memory_type_id, image_mem_reqs.size) }.unwrap();
        unsafe { device.bind_buffer_memory(&image_upload_memory, 0, &mut image_upload_buffer) }
            .unwrap();

        unsafe {
            let mut data = device
                .acquire_mapping_writer::<u8>(&image_upload_memory, 0..image_mem_reqs.size)
                .unwrap();
            for y in 0..height as usize {
                let row = &(*img)[y * (width as usize) * image_stride
                    ..(y + 1) * (width as usize) * image_stride];
                let dest_base = y * row_pitch as usize;
                data[dest_base..dest_base + row.len()].copy_from_slice(row);
            }
            device.release_mapping_writer(data).unwrap();
        }

        let mut image_logo = unsafe {
            device.create_image(
                kind,
                1,
                ColorFormat::SELF,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_DST | i::Usage::SAMPLED,
                i::ViewCapabilities::empty(),
            )
        }
        .unwrap();
        let image_req = unsafe { device.get_image_requirements(&image_logo) };
        let device_type = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                image_req.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::DEVICE_LOCAL)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();
        let image_memory = unsafe { device.allocate_memory(device_type, image_req.size) }.unwrap();

        unsafe { device.bind_image_memory(&image_memory, 0, &mut image_logo) }.unwrap();

        let image_srv = unsafe {
            device.create_image_view(
                &image_logo,
                i::ViewKind::D2,
                ColorFormat::SELF,
                Swizzle::NO,
                COLOR_RANGE.clone(),
            )
        }
        .unwrap();

        let sampler = unsafe {
            device.create_sampler(i::SamplerInfo::new(i::Filter::Linear, i::WrapMode::Clamp))
        }
        .expect("unable to make sampler");

        unsafe {
            device.write_descriptor_sets(vec![
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(&image_srv, i::Layout::Undefined)),
                },
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Sampler(&sampler)),
                },
            ])
        }

        let mut upload_fence = device.create_fence(false).expect("cant make fence");

        let cmd_buffer = unsafe {
            let mut cmd_buffer = self
                .command_pool
                .acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let image_barrier = m::Barrier::Image {
                states: (i::Access::empty(), i::Layout::Undefined)
                    ..(i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                target: &image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                i::Layout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: row_pitch / (image_stride as u32),
                    buffer_height: height as u32,
                    image_layers: i::SubresourceLayers {
                        aspects: f::Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: i::Offset { x: 0, y: 0, z: 0 },
                    image_extent: i::Extent {
                        width,
                        height,
                        depth: 1,
                    },
                }],
            );

            let image_barrier = m::Barrier::Image {
                states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                    ..(i::Access::SHADER_READ, i::Layout::ShaderReadOnlyOptimal),
                target: &image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };
            cmd_buffer.pipeline_barrier(
                PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.finish();

            self.queue_group.queues[0]
                .submit_nosemaphores(Some(&cmd_buffer), Some(&mut upload_fence));

            device
                .wait_for_fence(&upload_fence, u64::max_value())
                .expect("cant wait for fence");
            device.destroy_fence(upload_fence);

            cmd_buffer
        };

        // Compile shader modules
        let vs_module = {
            let glsl = VERTEX_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Vertex)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        let fs_module = {
            let glsl = FRAGMENT_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Fragment)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };

        // Describe the shaders
        const ENTRY_NAME: &str = "main";
        let vs_module: <back::Backend as Backend>::ShaderModule = vs_module;
        use hal::pso;
        let (vs_entry, fs_entry) = (
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &vs_module,
                specialization: pso::Specialization {
                    constants: &[pso::SpecializationConstant { id: 0, range: 0..4 }],
                    data: unsafe { std::mem::transmute::<&f32, &[u8; 4]>(&0.8f32) },
                },
            },
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &fs_module,
                specialization: pso::Specialization::default(),
            },
        );
        let shader_entries = pso::GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };

        // Create a render pass for this thing
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(self.format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Load,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: i::Access::empty()
                    ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            };

            unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
                .expect("Can't create render pass")
        };

        let subpass = Subpass {
            index: 0,
            main_pass: &render_pass,
        };

        // Create a descriptor set layout (this is mainly for textures), we just create an empty
        // one
        // let bindings = Vec::<pso::DescriptorSetLayoutBinding>::new();
        // let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
        // let set_layout = unsafe {
        //     device.create_descriptor_set_layout(bindings, immutable_samplers)
        // };

        // Create a pipeline layout
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                std::iter::once(&set_layout),
                // &[], // No descriptor set layout (no texture/sampler)
                &[(pso::ShaderStageFlags::VERTEX, 0..8)],
            )
        }
        .expect("Cant create pipelinelayout");

        // Describe the pipeline (rasterization, triangle interpretation)
        let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
            shader_entries,
            Primitive::TriangleList,
            pso::Rasterizer::FILL,
            &pipeline_layout,
            subpass,
        );

        pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
            binding: 0,
            stride: 16 as u32,
            rate: pso::VertexInputRate::Vertex,
            // 0 = Per Vertex
            // 1 = Per Instance
        });

        pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
            binding: 1,
            stride: 12 as u32,
            rate: pso::VertexInputRate::Instance(1), // VertexInputRate::Vertex,
                                                     // 0 = Per Vertex
                                                     // 1 = Per Instance
        });

        pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
            pso::ColorMask::ALL,
            pso::BlendState::ALPHA,
        ));

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 0,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        });

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 1,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 8,
            },
        });

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 2,
            binding: 1,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        });

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 3,
            binding: 1,
            element: pso::Element {
                format: f::Format::R32Sfloat,
                offset: 8,
            },
        });

        let pipeline = unsafe {
            device
                .create_graphics_pipeline(&pipeline_desc, None)
                .expect("Couldn't create a graphics pipeline!")
        };

        unsafe {
            device.destroy_shader_module(vs_module);
        }
        unsafe {
            device.destroy_shader_module(fs_module);
        }

        let memory_fence = device.create_fence(true).expect("memory fence");

        Bullets {
            buffer: vertex_buffer,
            buffer_size: instance_buffer_requirements.size,
            cmd_buffer: cmd_buffer,
            desc_set,
            device,
            image_upload_buffer,
            instance_buffer,
            instance_buffer_memory,
            instance_count: 2,
            memory: image_memory,
            memory_fence,
            pipeline,
            pipeline_layout,
            render_pass,
        }
    }

    pub fn create_static_texture_2d_rectangle<'b>(
        &mut self,
        device: &'b back::Device,
    ) -> StaticTexture2DRectangle<'b> {
        const VERTEX_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable

        layout(constant_id = 0) const float scale = 1.2f;

        layout(location = 0) in vec2 a_pos;
        layout(location = 1) in vec2 a_uv;
        layout(location = 0) out vec2 v_uv;

        out gl_PerVertex {
            vec4 gl_Position;
        };

        void main() {
            v_uv = a_uv;
            gl_Position = vec4(scale * a_pos, 0.0, 1.0);
        }";

        const FRAGMENT_SOURCE: &str = "#version 450
        #extension GL_ARB_separate_shader_objects : enable

        layout(location = 0) in vec2 v_uv;
        layout(location = 0) out vec4 target0;

        layout(set = 0, binding = 0) uniform texture2D u_texture;
        layout(set = 0, binding = 1) uniform sampler u_sampler;

        void main() {
            target0 = texture(sampler2D(u_texture, u_sampler), v_uv);
        }";
        let set_layout = unsafe {
            device.create_descriptor_set_layout(
                &[
                    pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::SampledImage,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    pso::DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: pso::DescriptorType::Sampler,
                        count: 1,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
                &[],
            )
        }
        .expect("Can't create descriptor set layout");

        // Descriptors
        let mut desc_pool = unsafe {
            device.create_descriptor_pool(
                1, // sets
                &[
                    pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::SampledImage,
                        count: 1,
                    },
                    pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Sampler,
                        count: 1,
                    },
                ],
                pso::DescriptorPoolCreateFlags::empty(),
            )
        }
        .expect("Can't create descriptor pool");
        let desc_set = unsafe { desc_pool.allocate_set(&set_layout) }.unwrap();

        // Allocate memory for Vertices and UV
        const F32_SIZE: u64 = std::mem::size_of::<f32>() as u64;
        const F32_PER_VERTEX: u64 = 2 + 2; // (x, y, u, v)
        const VERTICES: u64 = 6; // Using a triangle fan, which is the most optimal
        let mut vertex_buffer = unsafe {
            device.create_buffer(F32_SIZE * F32_PER_VERTEX * VERTICES, buffer::Usage::VERTEX)
        }
        .unwrap();
        let requirements = unsafe { device.get_buffer_requirements(&vertex_buffer) };

        use gfx_hal::{adapter::MemoryTypeId, memory::Properties};
        let memory_type_id = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::CPU_VISIBLE)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();

        let buffer_memory =
            unsafe { device.allocate_memory(memory_type_id, requirements.size) }.unwrap();
        unsafe { device.bind_buffer_memory(&buffer_memory, 0, &mut vertex_buffer) }.unwrap();
        unsafe {
            const QUAD: [f32; (F32_PER_VERTEX * VERTICES) as usize] = [
                -0.5, 0.33, 0.0, 1.0, 0.5, 0.33, 1.0, 1.0, 0.5, -0.33, 1.0, 0.0, -0.5, 0.33, 0.0,
                1.0, 0.5, -0.33, 1.0, 0.0, -0.5, -0.33, 0.0, 0.0,
            ];
            let mut vertices = device
                .acquire_mapping_writer::<f32>(&buffer_memory, 0..requirements.size)
                .unwrap();
            vertices[0..QUAD.len()].copy_from_slice(&QUAD);
            device.release_mapping_writer(vertices).unwrap();
        }

        let img_data = include_bytes!["data/logo.png"];
        let img = image::load(Cursor::new(&img_data[..]), image::PNG)
            .unwrap()
            .to_rgba();
        let (width, height) = img.dimensions();
        let kind = i::Kind::D2(width as i::Size, height as i::Size, 1, 1);
        let limits = self.adapter.physical_device.limits();
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;
        let image_stride = 4usize;
        let row_pitch = (width * image_stride as u32 + row_alignment_mask) & !row_alignment_mask;
        let upload_size = (height * row_pitch) as u64;

        let mut image_upload_buffer =
            unsafe { device.create_buffer(upload_size, buffer::Usage::TRANSFER_SRC) }.unwrap();
        let image_mem_reqs = unsafe { device.get_buffer_requirements(&image_upload_buffer) };
        let image_upload_memory =
            unsafe { device.allocate_memory(memory_type_id, image_mem_reqs.size) }.unwrap();
        unsafe { device.bind_buffer_memory(&image_upload_memory, 0, &mut image_upload_buffer) }
            .unwrap();

        unsafe {
            let mut data = device
                .acquire_mapping_writer::<u8>(&image_upload_memory, 0..image_mem_reqs.size)
                .unwrap();
            for y in 0..height as usize {
                let row = &(*img)[y * (width as usize) * image_stride
                    ..(y + 1) * (width as usize) * image_stride];
                let dest_base = y * row_pitch as usize;
                data[dest_base..dest_base + row.len()].copy_from_slice(row);
            }
            device.release_mapping_writer(data).unwrap();
        }

        let mut image_logo = unsafe {
            device.create_image(
                kind,
                1,
                ColorFormat::SELF,
                i::Tiling::Optimal,
                i::Usage::TRANSFER_DST | i::Usage::SAMPLED,
                i::ViewCapabilities::empty(),
            )
        }
        .unwrap();
        let image_req = unsafe { device.get_image_requirements(&image_logo) };
        let device_type = self
            .adapter
            .physical_device
            .memory_properties()
            .memory_types
            .iter()
            .enumerate()
            .find(|&(id, memory_type)| {
                image_req.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(Properties::DEVICE_LOCAL)
            })
            .map(|(id, _)| MemoryTypeId(id))
            .unwrap();
        let image_memory = unsafe { device.allocate_memory(device_type, image_req.size) }.unwrap();

        unsafe { device.bind_image_memory(&image_memory, 0, &mut image_logo) }.unwrap();

        let image_srv = unsafe {
            device.create_image_view(
                &image_logo,
                i::ViewKind::D2,
                ColorFormat::SELF,
                Swizzle::NO,
                COLOR_RANGE.clone(),
            )
        }
        .unwrap();

        let sampler = unsafe {
            device.create_sampler(i::SamplerInfo::new(i::Filter::Linear, i::WrapMode::Clamp))
        }
        .expect("unable to make sampler");

        unsafe {
            device.write_descriptor_sets(vec![
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(&image_srv, i::Layout::Undefined)),
                },
                pso::DescriptorSetWrite {
                    set: &desc_set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Sampler(&sampler)),
                },
            ])
        }

        let mut upload_fence = device.create_fence(false).expect("cant make fence");

        let cmd_buffer = unsafe {
            let mut cmd_buffer = self
                .command_pool
                .acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let image_barrier = m::Barrier::Image {
                states: (i::Access::empty(), i::Layout::Undefined)
                    ..(i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal),
                target: &image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.copy_buffer_to_image(
                &image_upload_buffer,
                &image_logo,
                i::Layout::TransferDstOptimal,
                &[command::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: row_pitch / (image_stride as u32),
                    buffer_height: height as u32,
                    image_layers: i::SubresourceLayers {
                        aspects: f::Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: i::Offset { x: 0, y: 0, z: 0 },
                    image_extent: i::Extent {
                        width,
                        height,
                        depth: 1,
                    },
                }],
            );

            let image_barrier = m::Barrier::Image {
                states: (i::Access::TRANSFER_WRITE, i::Layout::TransferDstOptimal)
                    ..(i::Access::SHADER_READ, i::Layout::ShaderReadOnlyOptimal),
                target: &image_logo,
                families: None,
                range: COLOR_RANGE.clone(),
            };
            cmd_buffer.pipeline_barrier(
                PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                m::Dependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.finish();

            self.queue_group.queues[0]
                .submit_nosemaphores(Some(&cmd_buffer), Some(&mut upload_fence));

            device
                .wait_for_fence(&upload_fence, u64::max_value())
                .expect("cant wait for fence");
            device.destroy_fence(upload_fence);

            cmd_buffer
        };

        // Compile shader modules
        let vs_module = {
            let glsl = VERTEX_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Vertex)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        let fs_module = {
            let glsl = FRAGMENT_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Fragment)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };

        // Describe the shaders
        const ENTRY_NAME: &str = "main";
        let vs_module: <back::Backend as Backend>::ShaderModule = vs_module;
        use hal::pso;
        let (vs_entry, fs_entry) = (
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &vs_module,
                specialization: pso::Specialization {
                    constants: &[pso::SpecializationConstant { id: 0, range: 0..4 }],
                    data: unsafe { std::mem::transmute::<&f32, &[u8; 4]>(&0.8f32) },
                },
            },
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &fs_module,
                specialization: pso::Specialization::default(),
            },
        );
        let shader_entries = pso::GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };

        // Create a render pass for this thing
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(self.format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Load,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: i::Access::empty()
                    ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            };

            unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
                .expect("Can't create render pass")
        };

        let subpass = Subpass {
            index: 0,
            main_pass: &render_pass,
        };

        // Create a descriptor set layout (this is mainly for textures), we just create an empty
        // one
        // let bindings = Vec::<pso::DescriptorSetLayoutBinding>::new();
        // let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
        // let set_layout = unsafe {
        //     device.create_descriptor_set_layout(bindings, immutable_samplers)
        // };

        // Create a pipeline layout
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                std::iter::once(&set_layout),
                // &[], // No descriptor set layout (no texture/sampler)
                &[(pso::ShaderStageFlags::VERTEX, 0..8)],
            )
        }
        .expect("Cant create pipelinelayout");

        // Describe the pipeline (rasterization, triangle interpretation)
        let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
            shader_entries,
            Primitive::TriangleList,
            pso::Rasterizer::FILL,
            &pipeline_layout,
            subpass,
        );

        pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
            binding: 0,
            stride: 16 as u32,
            rate: pso::VertexInputRate::Vertex,
            // 0 = Per Vertex
            // 1 = Per Instance
        });

        pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
            pso::ColorMask::ALL,
            pso::BlendState::ALPHA,
        ));

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 0,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        });

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 1,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 8,
            },
        });

        let pipeline = unsafe {
            device
                .create_graphics_pipeline(&pipeline_desc, None)
                .expect("Couldn't create a graphics pipeline!")
        };

        unsafe {
            device.destroy_shader_module(vs_module);
        }
        unsafe {
            device.destroy_shader_module(fs_module);
        }

        let memory_fence = device.create_fence(false).expect("memory fence");

        StaticTexture2DRectangle {
            buffer: vertex_buffer,
            cmd_buffer: cmd_buffer,
            device,
            image_upload_buffer,
            memory: image_memory,
            memory_fence,
            pipeline,
            render_pass,
        }
    }

    pub fn create_static_white_2d_triangle(
        &mut self,
        device: &back::Device,
        triangle: &[f32; 6],
    ) -> StaticWhite2DTriangle {
        pub const VERTEX_SOURCE: &str = "#version 450
        #extension GL_ARG_separate_shader_objects : enable
        layout (location = 0) in vec2 position;
        out gl_PerVertex {
          vec4 gl_Position;
        };
        void main()
        {
          gl_Position = vec4(position, 0.0, 1.0);
        }";

        pub const FRAGMENT_SOURCE: &str = "#version 450
        #extension GL_ARG_separate_shader_objects : enable
        layout(location = 0) out vec4 color;
        void main()
        {
          color = vec4(1.0);
        }";

        // Create a buffer for the vertex data (this is rather involved)
        let (buffer, memory, requirements) = unsafe {
            const F32_XY_TRIANGLE: u64 = (std::mem::size_of::<f32>() * 2 * 3) as u64;
            use gfx_hal::{adapter::MemoryTypeId, memory::Properties};
            let mut buffer = device
                .create_buffer(F32_XY_TRIANGLE, gfx_hal::buffer::Usage::VERTEX)
                .expect("cant make bf");
            let requirements = device.get_buffer_requirements(&buffer);
            let memory_type_id = self
                .adapter
                .physical_device
                .memory_properties()
                .memory_types
                .iter()
                .enumerate()
                .find(|&(id, memory_type)| {
                    requirements.type_mask & (1 << id) != 0
                        && memory_type.properties.contains(Properties::CPU_VISIBLE)
                })
                .map(|(id, _)| MemoryTypeId(id))
                .unwrap();
            let memory = device
                .allocate_memory(memory_type_id, requirements.size)
                .expect("Couldn't allocate vertex buffer memory");
            println!["{:?}", memory];
            device
                .bind_buffer_memory(&memory, 0, &mut buffer)
                .expect("Couldn't bind the buffer memory!");
            // (buffer, memory, requirements)
            (buffer, memory, requirements)
        };

        // Upload vertex data
        unsafe {
            let mut data_target = self
                .device
                .acquire_mapping_writer(&memory, 0..requirements.size)
                .expect("Failed to acquire a memory writer!");
            let points = triangle;
            println!["Uploading points: {:?}", points];
            data_target[..points.len()].copy_from_slice(points);
            self.device
                .release_mapping_writer(data_target)
                .expect("Couldn't release the mapping writer!");
        }

        // Compile shader modules
        let vs_module = {
            let glsl = VERTEX_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Vertex)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };
        let fs_module = {
            let glsl = FRAGMENT_SOURCE;
            let spirv: Vec<u8> = glsl_to_spirv::compile(&glsl, glsl_to_spirv::ShaderType::Fragment)
                .unwrap()
                .bytes()
                .map(|b| b.unwrap())
                .collect();
            unsafe { device.create_shader_module(&spirv) }.unwrap()
        };

        // Describe the shaders
        const ENTRY_NAME: &str = "main";
        let vs_module: <back::Backend as Backend>::ShaderModule = vs_module;
        use hal::pso;
        let (vs_entry, fs_entry) = (
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &vs_module,
                // specialization: pso::Specialization {
                //     constants: &[pso::SpecializationConstant { id: 0, range: 0..4 }],
                //     data: unsafe { std::mem::transmute::<&f32, &[u8; 4]>(&0.8f32) },
                // },
                specialization: pso::Specialization::default(),
            },
            pso::EntryPoint {
                entry: ENTRY_NAME,
                module: &fs_module,
                specialization: pso::Specialization::default(),
            },
        );
        let shader_entries = pso::GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };

        // Create a render pass for this thing
        let render_pass = {
            let attachment = pass::Attachment {
                format: Some(self.format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Load,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            // let dependency = pass::SubpassDependency {
            //     passes: pass::SubpassRef::External..pass::SubpassRef::Pass(0),
            //     stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
            //         ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            //     accesses: i::Access::empty()
            //         ..(i::Access::COLOR_ATTACHMENT_READ | i::Access::COLOR_ATTACHMENT_WRITE),
            // };

            unsafe { device.create_render_pass(&[attachment], &[subpass], &[]) }
                .expect("Can't create render pass")
        };

        let subpass = Subpass {
            index: 0,
            main_pass: &render_pass,
        };

        // Create a descriptor set layout (this is mainly for textures), we just create an empty
        // one
        // let bindings = Vec::<pso::DescriptorSetLayoutBinding>::new();
        // let immutable_samplers = Vec::<<back::Backend as Backend>::Sampler>::new();
        // let set_layout = unsafe {
        //     device.create_descriptor_set_layout(bindings, immutable_samplers)
        // };

        // Create a pipeline layout
        let pipeline_layout = unsafe {
            device.create_pipeline_layout(
                // &set_layout,
                &[], // No descriptor set layout (no texture/sampler)
                &[], // &[(pso::ShaderStageFlags::VERTEX, 0..4)],
            )
        }
        .expect("Cant create pipelinelayout");

        // Describe the pipeline (rasterization, triangle interpretation)
        let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
            shader_entries,
            Primitive::TriangleList,
            pso::Rasterizer::FILL,
            &pipeline_layout,
            subpass,
        );

        pipeline_desc.vertex_buffers.push(pso::VertexBufferDesc {
            binding: 0,
            stride: 8 as u32,
            rate: pso::VertexInputRate::Vertex,
            // 0 = Per Vertex
            // 1 = Per Instance
        });

        pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
            pso::ColorMask::ALL,
            pso::BlendState::ALPHA,
        ));

        pipeline_desc.attributes.push(pso::AttributeDesc {
            location: 0,
            binding: 0,
            element: pso::Element {
                format: f::Format::Rg32Sfloat,
                offset: 0,
            },
        });

        let pipeline = unsafe {
            device
                .create_graphics_pipeline(&pipeline_desc, None)
                .expect("Couldn't create a graphics pipeline!")
        };

        unsafe {
            device.destroy_shader_module(vs_module);
        }
        unsafe {
            device.destroy_shader_module(fs_module);
        }

        let cmd_buffer = self
            .command_pool
            .acquire_command_buffer::<command::MultiShot>();

        let memory_fence = device.create_fence(false).expect("Unable to make fence");

        StaticWhite2DTriangle {
            buffer,
            cmd_buffer,
            memory,
            memory_fence,
            pipeline,
            render_pass,
        }
    }
    fn clear(&mut self, frame: hal::SwapImageIndex, r: f32) {
        let render_pass = {
            let color_attachment = pass::Attachment {
                format: Some(self.format),
                samples: 1,
                ops: pass::AttachmentOps {
                    load: pass::AttachmentLoadOp::Clear,
                    store: pass::AttachmentStoreOp::Store,
                },
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: i::Layout::Undefined..i::Layout::Present,
            };
            let subpass = pass::SubpassDesc {
                colors: &[(0, i::Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };
            unsafe {
                self.device
                    .create_render_pass(&[color_attachment], &[subpass], &[])
                    .map_err(|_| "Couldn't create a render pass!")
                    .unwrap()
            }
        };
        let mut cmd_buffer = self
            .command_pool
            .acquire_command_buffer::<command::OneShot>();
        unsafe {
            cmd_buffer.begin();

            cmd_buffer.set_viewports(0, &[self.viewport.clone()]);
            cmd_buffer.set_scissors(0, &[self.viewport.rect]);
            // cmd_buffer.bind_graphics_pipeline(&self.pipeline);
            // cmd_buffer.bind_vertex_buffers(0, Some((&self.vertex_buffer, 0)));
            // cmd_buffer.bind_graphics_descriptor_sets(&self.pipeline_layout, 0, Some(&self.desc_set), &[]);

            cmd_buffer.begin_render_pass_inline(
                &render_pass,
                &self.framebuffers[frame as usize],
                self.viewport.rect,
                &[command::ClearValue::Color(command::ClearColor::Float([
                    r, 0.8, 0.8, 1.0,
                ]))],
            );

            cmd_buffer.finish();

            let submission = Submission {
                command_buffers: Some(&cmd_buffer),
                wait_semaphores: Some((
                    &self.frame_semaphore[self.frame_index],
                    PipelineStage::BOTTOM_OF_PIPE,
                )),
                signal_semaphores: None, // Some(&self.render_finished_semaphore[self.frame_index]),
            };
            // self.queue_group.queues[0].submit(submission, Some(&mut self.frame_fence));
            self.queue_group.queues[0].submit(submission, Some(&mut self.frame_fence[self.frame_index]));
            self.device
                .wait_for_fence(&self.frame_fence[self.frame_index], 100_000_000)
                .expect("Unable to wait on fence");
            self.device
                .reset_fence(&self.frame_fence[self.frame_index])
                .expect("Unable to reset fence");
        }
    }
}