//! Automatic, conservative tuning based on the device's physical RAM.
//!
//! The JVM heap is the single biggest lever on low-end phones, and the GPU is
//! the second: Mali-based Exynos devices are happiest with gl4es, while Zink
//! should only be enabled manually on Vulkan-capable Adreno devices.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Renderer {
    /// GL 2.1 -> GLES 2/3. Safest on Mali, lowest memory.
    Gl4es,
    /// GLES -> Vulkan. Good compatibility on modern Adreno.
    Angle,
    /// GL -> Vulkan via Mesa. Heaviest; avoid on Mali.
    Zink,
}

impl Renderer {
    pub fn library(self) -> &'static str {
        match self {
            Renderer::Gl4es => "libgl4es.so",
            Renderer::Angle => "libEGL_angle.so",
            Renderer::Zink => "libGL_zink.so",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Renderer::Gl4es => "gl4es",
            Renderer::Angle => "ANGLE",
            Renderer::Zink => "Zink",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceProfile {
    pub total_ram_mb: u32,
    pub heap_mb: u32,
    pub min_heap_mb: u32,
    pub renderer: Renderer,
    pub render_distance: u32,
    pub resolution_scale: f32,
}

impl DeviceProfile {
    /// Reads `/proc/meminfo`. On Android this is the physical RAM visible to
    /// the app, which is a bit lower than the marketed number (6 GB -> ~5.4).
    pub fn detect() -> Self {
        Self::for_ram(detect_ram_mb().unwrap_or(3072))
    }

    pub fn for_ram(total_ram_mb: u32) -> Self {
        let (heap_mb, render_distance, resolution_scale) = match total_ram_mb {
            0..=2499 => (384, 4, 0.75),
            2500..=3499 => (512, 5, 0.8),
            3500..=4599 => (768, 6, 0.85),
            4600..=6499 => (1024, 8, 0.9),
            6500..=8999 => (1536, 10, 1.0),
            _ => (2048, 12, 1.0),
        };

        DeviceProfile {
            total_ram_mb,
            heap_mb,
            min_heap_mb: (heap_mb / 2).max(256),
            renderer: Renderer::Gl4es,
            render_distance,
            resolution_scale,
        }
    }

    pub fn with_renderer(mut self, renderer: Renderer) -> Self {
        self.renderer = renderer;
        self
    }

    pub fn jvm_heap_args(&self) -> Vec<String> {
        vec![
            format!("-Xmx{}M", self.heap_mb),
            format!("-Xms{}M", self.min_heap_mb),
        ]
    }

    /// Human readable one-liner for the UI / logs.
    pub fn describe(&self) -> String {
        format!(
            "{} MB RAM -> heap {} MB, {}, render distance {}, scale {:.2}",
            self.total_ram_mb,
            self.heap_mb,
            self.renderer.label(),
            self.render_distance,
            self.resolution_scale
        )
    }
}

pub fn detect_ram_mb() -> Option<u32> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u32 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samsung_m33_5g_6gb() {
        // 6 GB marketed, ~5.4 GB reported by the kernel.
        let profile = DeviceProfile::for_ram(5400);
        assert_eq!(profile.heap_mb, 1024);
        assert_eq!(profile.min_heap_mb, 512);
        assert_eq!(profile.renderer, Renderer::Gl4es);
        assert_eq!(profile.render_distance, 8);
    }

    #[test]
    fn tiny_device_stays_alive() {
        let profile = DeviceProfile::for_ram(1900);
        assert_eq!(profile.heap_mb, 384);
        assert!(profile.min_heap_mb >= 256);
        assert_eq!(profile.resolution_scale, 0.75);
    }

    #[test]
    fn heap_args_match_fields() {
        let profile = DeviceProfile::for_ram(5400);
        assert_eq!(
            profile.jvm_heap_args(),
            vec!["-Xmx1024M".to_string(), "-Xms512M".to_string()]
        );
    }

    #[test]
    fn renderer_override() {
        let profile = DeviceProfile::for_ram(8000).with_renderer(Renderer::Angle);
        assert_eq!(profile.renderer.library(), "libEGL_angle.so");
    }
}
