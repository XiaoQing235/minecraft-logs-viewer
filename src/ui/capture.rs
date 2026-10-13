use fltk::{prelude::*, window::Window};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

pub fn window(window: &mut Window, destination: &Path) -> io::Result<()> {
    let surface = fltk::surface::ImageSurface::new(window.width(), window.height(), false);
    fltk::surface::ImageSurface::push_current(&surface);
    surface.draw(window, 0, 0);
    let image = surface.image();
    fltk::surface::ImageSurface::pop_current();
    let image = image.ok_or_else(|| io::Error::other("无法取得窗口绘制图像"))?;
    let width = image.data_w() as usize;
    let height = image.data_h() as usize;
    let rgb = image.to_rgb_data();
    if rgb.len() != width * height * 3 {
        return Err(io::Error::other("窗口截图不是 RGB 数据"));
    }
    let stride = (width * 3 + 3) & !3;
    let size = stride * height;
    let mut file = File::create(destination)?;
    file.write_all(b"BM")?;
    file.write_all(&((54 + size) as u32).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54u32.to_le_bytes())?;
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&(width as i32).to_le_bytes())?;
    file.write_all(&(height as i32).to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&24u16.to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&(size as u32).to_le_bytes())?;
    file.write_all(&[0; 16])?;
    let mut row = vec![0; stride];
    for y in (0..height).rev() {
        for x in 0..width {
            let offset = (y * width + x) * 3;
            row[x * 3..x * 3 + 3].copy_from_slice(&[rgb[offset + 2], rgb[offset + 1], rgb[offset]]);
        }
        file.write_all(&row)?;
    }
    file.flush()
}
