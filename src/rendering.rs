#![warn(dead_code)]
#![allow(unused_braces)]
#![warn(unused)]

#[derive(Default, Clone, Copy)]
pub struct RenderingManager {
    rendering: bool,
    pub w: u16,
    pub h: u16,
    fps: i16,
    interval: u64
}

impl RenderingManager {
    pub fn new(w: u16, h: u16, fps: i16) -> Self {
        RenderingManager {
            rendering: RenderingManager::check_from(w, h),
            w,
            h,
            fps,
            interval: RenderingManager::calculate_interval(fps)
        }
    }

    pub fn calculate_interval(fps: i16) -> u64 {
        (1000 / fps) as u64
    }

    pub fn new_fps(&mut self, fps: i16) -> u64 {
        self.fps = fps;
        self.interval = RenderingManager::calculate_interval(fps);
        self.interval
    }
    
    pub fn get_fps_interval(&mut self) -> u64 {
        self.interval
    }
    
    pub fn resize(&mut self, w: u16, h: u16) -> (u16, u16) {
        self.w = w;
        self.h = h;
        (w, h)
    }

    pub fn check(&mut self) -> bool {
        self.rendering = RenderingManager::check_from(self.w, self.h);
        self.rendering
    }
    
    pub fn check_from(w: u16, h: u16) -> bool {
        w > 20 && h > 20
    }

    pub fn is_enabled(&mut self) -> bool {
        self.rendering
    }
    
    // pub fn toggle(&mut self) -> bool {
    //     self.rendering = !self.rendering;
    //     self.rendering
    // }

    pub fn set(&mut self, new: bool) -> bool {
        self.rendering = new;
        new
    }
}
