use image::RgbaImage;
use std::{mem, ptr};
use super::libobs;

pub struct Texture {
    texture: *mut libobs::gs_texture_t,
    width: u32,
    height: u32,
}
impl Texture {
    pub fn new(image: &RgbaImage) -> Self {
        let (width, height) = image.dimensions();
        unsafe {
            let mut scans = vec![ptr::null() as *const u8; height as usize];
            for i in 0..height {
                scans[i as usize] = mem::transmute(image.get_pixel(0, i));
            }

            let texture = libobs::gs_texture_create(
                width,
                height,
                libobs::gs_color_format::GS_RGBA,
                1,
                scans.as_ptr() as *mut *const u8,
                0,
            );
            Texture {
                texture: texture,
                width: width,
                height: height,
            }
        }
    }
    pub fn draw(&self) {
        unsafe {
            libobs::obs_source_draw(self.texture, 0, 0, 0, 0, false);
        }
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}
impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            libobs::gs_texture_destroy(self.texture);
        }
    }
}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}
