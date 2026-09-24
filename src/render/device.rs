#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(u64);

impl BufferHandle {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(u64);

impl TextureHandle {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    Vertex,
    Index,
    Uniform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferDescriptor {
    pub size: usize,
    pub usage: BufferUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDescriptor {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    InvalidResource,
    UnsupportedFormat,
    UnsupportedCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    WriteBuffer {
        buffer: BufferHandle,
        offset: usize,
        bytes: Vec<u8>,
    },
    WriteTexture {
        texture: TextureHandle,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    },
    DrawTexture {
        texture: TextureHandle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        opacity_bits: u32,
    },
    Present,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandList {
    commands: Vec<Command>,
}

impl CommandList {
    pub fn write_buffer(&mut self, buffer: BufferHandle, offset: usize, bytes: Vec<u8>) {
        self.commands.push(Command::WriteBuffer {
            buffer,
            offset,
            bytes,
        });
    }

    pub fn write_texture(
        &mut self,
        texture: TextureHandle,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    ) {
        self.commands.push(Command::WriteTexture {
            texture,
            width,
            height,
            bytes,
        });
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn draw_texture(
        &mut self,
        texture: TextureHandle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        opacity: f32,
    ) {
        self.commands.push(Command::DrawTexture {
            texture,
            x,
            y,
            width,
            height,
            opacity_bits: opacity.to_bits(),
        });
    }

    pub fn present(&mut self) {
        self.commands.push(Command::Present);
    }
}

pub trait GraphicsDevice: Send {
    fn resize(&mut self, _viewport: crate::render::Viewport) -> Result<(), DeviceError> {
        Ok(())
    }

    fn create_buffer(&mut self, descriptor: BufferDescriptor) -> Result<BufferHandle, DeviceError>;
    fn create_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureHandle, DeviceError>;
    fn submit(&mut self, commands: CommandList) -> Result<(), DeviceError>;
    fn destroy_buffer(&mut self, buffer: BufferHandle);
    fn destroy_texture(&mut self, texture: TextureHandle);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedTexture {
    handle: TextureHandle,
    revision: ImageRevision,
    dimensions: (u32, u32),
}

#[derive(Debug, Default)]
pub struct TextureResourceCache {
    textures: HashMap<ImageId, CachedTexture>,
}

impl TextureResourceCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&self, image: ImageId) -> Option<TextureHandle> {
        self.textures.get(&image).map(|texture| texture.handle)
    }

    pub fn upload_updates<D: GraphicsDevice>(
        &mut self,
        device: &mut D,
        updates: &[ImageUpdate],
    ) -> Result<usize, DeviceError> {
        let mut commands = CommandList::default();
        let mut staged_textures = self.textures.clone();
        let mut created_handles = Vec::new();
        let mut replaced_handles = Vec::new();
        let mut uploaded = 0;
        let staging_result = (|| {
            for update in updates {
                let dimensions = update.dimensions();
                if staged_textures
                    .get(&update.image())
                    .is_some_and(|cached| cached.revision.value() >= update.revision().value())
                {
                    continue;
                }

                let (handle, replaced_handle) = match staged_textures.get(&update.image()) {
                    Some(cached) if cached.dimensions == dimensions => (cached.handle, None),
                    cached => {
                        let handle = device.create_texture(TextureDescriptor {
                            width: dimensions.0,
                            height: dimensions.1,
                            format: TextureFormat::Rgba8,
                        })?;
                        created_handles.push(handle);
                        (handle, cached.map(|cached| cached.handle))
                    }
                };
                commands.write_texture(handle, dimensions.0, dimensions.1, update.rgba8().to_vec());
                staged_textures.insert(
                    update.image(),
                    CachedTexture {
                        handle,
                        revision: update.revision(),
                        dimensions,
                    },
                );
                if let Some(replaced_handle) = replaced_handle {
                    replaced_handles.push(replaced_handle);
                }
                uploaded += 1;
            }
            Ok::<(), DeviceError>(())
        })();
        if let Err(error) = staging_result {
            for handle in created_handles {
                device.destroy_texture(handle);
            }
            return Err(error);
        }
        if uploaded > 0 {
            if let Err(error) = device.submit(commands) {
                for handle in created_handles {
                    device.destroy_texture(handle);
                }
                return Err(error);
            }
        }
        self.textures = staged_textures;
        for handle in replaced_handles {
            device.destroy_texture(handle);
        }
        Ok(uploaded)
    }

    pub fn evict<D: GraphicsDevice>(&mut self, device: &mut D, images: &[ImageId]) {
        for image in images {
            if let Some(texture) = self.textures.remove(image) {
                device.destroy_texture(texture.handle);
            }
        }
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub struct MockGraphicsDevice {
    next_handle: u64,
    fail_next_texture_creation: bool,
    fail_next_submission: bool,
    buffers: HashMap<BufferHandle, BufferDescriptor>,
    textures: HashMap<TextureHandle, TextureDescriptor>,
    submitted: Vec<CommandList>,
}

#[cfg(test)]
impl MockGraphicsDevice {
    fn next_handle(&mut self) -> u64 {
        self.next_handle = self.next_handle.wrapping_add(1);
        self.next_handle
    }

    pub fn submitted(&self) -> &[CommandList] {
        &self.submitted
    }
}

#[cfg(test)]
impl GraphicsDevice for MockGraphicsDevice {
    fn create_buffer(&mut self, descriptor: BufferDescriptor) -> Result<BufferHandle, DeviceError> {
        if descriptor.size == 0 {
            return Err(DeviceError::InvalidResource);
        }
        let handle = BufferHandle::new(self.next_handle());
        self.buffers.insert(handle, descriptor);
        Ok(handle)
    }

    fn create_texture(
        &mut self,
        descriptor: TextureDescriptor,
    ) -> Result<TextureHandle, DeviceError> {
        if std::mem::take(&mut self.fail_next_texture_creation) {
            return Err(DeviceError::InvalidResource);
        }
        if descriptor.width == 0 || descriptor.height == 0 {
            return Err(DeviceError::InvalidResource);
        }
        let handle = TextureHandle::new(self.next_handle());
        self.textures.insert(handle, descriptor);
        Ok(handle)
    }

    fn submit(&mut self, commands: CommandList) -> Result<(), DeviceError> {
        if std::mem::take(&mut self.fail_next_submission) {
            return Err(DeviceError::UnsupportedCommand);
        }
        for (index, command) in commands.commands().iter().enumerate() {
            match command {
                Command::WriteBuffer {
                    buffer,
                    offset,
                    bytes,
                } => {
                    let descriptor = self
                        .buffers
                        .get(buffer)
                        .ok_or(DeviceError::InvalidResource)?;
                    if offset
                        .checked_add(bytes.len())
                        .is_none_or(|end| end > descriptor.size)
                    {
                        return Err(DeviceError::InvalidResource);
                    }
                }
                Command::WriteTexture {
                    texture,
                    width,
                    height,
                    bytes,
                } => {
                    let descriptor = self
                        .textures
                        .get(texture)
                        .ok_or(DeviceError::InvalidResource)?;
                    let expected = (*width as usize)
                        .checked_mul(*height as usize)
                        .and_then(|pixels| pixels.checked_mul(4));
                    if (*width, *height) != (descriptor.width, descriptor.height)
                        || expected != Some(bytes.len())
                    {
                        return Err(DeviceError::InvalidResource);
                    }
                }
                Command::DrawTexture {
                    texture,
                    width,
                    height,
                    opacity_bits,
                    ..
                } => {
                    if !self.textures.contains_key(texture)
                        || *width == 0
                        || *height == 0
                        || !(0.0..=1.0).contains(&f32::from_bits(*opacity_bits))
                    {
                        return Err(DeviceError::InvalidResource);
                    }
                }
                Command::Present if index + 1 != commands.commands().len() => {
                    return Err(DeviceError::UnsupportedCommand);
                }
                Command::Present => {}
            }
        }
        self.submitted.push(commands);
        Ok(())
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) {
        self.buffers.remove(&buffer);
    }

    fn destroy_texture(&mut self, texture: TextureHandle) {
        self.textures.remove(&texture);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BufferDescriptor, BufferUsage, Command, CommandList, DeviceError, GraphicsDevice,
        MockGraphicsDevice, TextureDescriptor, TextureFormat, TextureResourceCache,
    };

    #[test]
    fn mock_graphics_device_validates_resource_lifecycle_and_command_order() {
        let mut mock = MockGraphicsDevice::default();
        let texture = mock
            .create_texture(TextureDescriptor {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8,
            })
            .unwrap();
        let mut commands = CommandList::default();
        commands.write_texture(texture, 1, 1, vec![0, 0, 0, 255]);
        commands.draw_texture(texture, 0, 0, 1, 1, 1.0);
        commands.present();

        mock.submit(commands).unwrap();
        assert_eq!(mock.submitted().len(), 1);

        mock.destroy_texture(texture);
        let mut stale_draw = CommandList::default();
        stale_draw.draw_texture(texture, 0, 0, 1, 1, 1.0);
        stale_draw.present();
        assert_eq!(mock.submit(stale_draw), Err(DeviceError::InvalidResource));
    }

    #[test]
    fn mock_graphics_device_rejects_out_of_bounds_writes_and_commands_after_present() {
        let mut mock = MockGraphicsDevice::default();
        let buffer = mock
            .create_buffer(BufferDescriptor {
                size: 4,
                usage: BufferUsage::Vertex,
            })
            .unwrap();
        let texture = mock
            .create_texture(TextureDescriptor {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8,
            })
            .unwrap();

        let mut out_of_bounds = CommandList::default();
        out_of_bounds.write_buffer(buffer, 2, vec![0; 3]);
        assert_eq!(
            mock.submit(out_of_bounds),
            Err(DeviceError::InvalidResource)
        );

        let mut after_present = CommandList::default();
        after_present.present();
        after_present.draw_texture(texture, 0, 0, 1, 1, 1.0);
        assert_eq!(
            mock.submit(after_present),
            Err(DeviceError::UnsupportedCommand)
        );
    }

    #[test]
    fn device_contract_keeps_resource_creation_and_submission_backend_independent() {
        let mut device = MockGraphicsDevice::default();
        let buffer = device
            .create_buffer(BufferDescriptor {
                size: 16,
                usage: BufferUsage::Vertex,
            })
            .unwrap();
        let texture = device
            .create_texture(TextureDescriptor {
                width: 2,
                height: 2,
                format: TextureFormat::Rgba8,
            })
            .unwrap();

        let mut commands = CommandList::default();
        commands.write_buffer(buffer, 4, vec![1, 2, 3]);
        commands.write_texture(texture, 2, 2, vec![255; 16]);
        device.submit(commands).unwrap();

        assert_eq!(buffer.value(), 1);
        assert_eq!(texture.value(), 2);
        assert_eq!(device.submitted().len(), 1);
        assert!(matches!(
            &device.submitted()[0].commands()[0],
            Command::WriteBuffer { offset: 4, .. }
        ));
    }

    #[test]
    fn texture_cache_uploads_only_new_revisions_and_reuses_same_dimensions() {
        let mut device = MockGraphicsDevice::default();
        let mut cache = TextureResourceCache::new();
        let first = crate::render::ImageUpdate::new(
            crate::render::ImageId::new(7),
            crate::render::ImageRevision::new(1),
            1,
            1,
            vec![1, 2, 3, 255],
        )
        .unwrap();
        let newer = crate::render::ImageUpdate::new(
            crate::render::ImageId::new(7),
            crate::render::ImageRevision::new(2),
            1,
            1,
            vec![4, 5, 6, 255],
        )
        .unwrap();

        assert_eq!(
            cache.upload_updates(&mut device, &[first.clone()]).unwrap(),
            1
        );
        let handle = cache.handle(crate::render::ImageId::new(7));
        assert_eq!(cache.upload_updates(&mut device, &[first]).unwrap(), 0);
        assert_eq!(cache.upload_updates(&mut device, &[newer]).unwrap(), 1);
        assert_eq!(cache.handle(crate::render::ImageId::new(7)), handle);
        assert_eq!(device.submitted().len(), 2);
    }

    #[test]
    fn texture_cache_keeps_old_texture_when_replacement_creation_fails() {
        let image = crate::render::ImageId::new(18);
        let mut device = MockGraphicsDevice::default();
        let mut cache = TextureResourceCache::new();
        let first = crate::render::ImageUpdate::new(
            image,
            crate::render::ImageRevision::new(1),
            1,
            1,
            vec![1, 2, 3, 255],
        )
        .unwrap();
        cache.upload_updates(&mut device, &[first]).unwrap();
        let old_handle = cache.handle(image).unwrap();

        device.fail_next_texture_creation = true;
        let resized = crate::render::ImageUpdate::new(
            image,
            crate::render::ImageRevision::new(2),
            2,
            1,
            vec![4, 5, 6, 255, 7, 8, 9, 255],
        )
        .unwrap();

        assert_eq!(
            cache.upload_updates(&mut device, &[resized]),
            Err(DeviceError::InvalidResource)
        );
        assert_eq!(cache.handle(image), Some(old_handle));
        assert!(device.textures.contains_key(&old_handle));
    }

    #[test]
    fn texture_cache_keeps_old_texture_when_replacement_upload_fails() {
        let image = crate::render::ImageId::new(19);
        let mut device = MockGraphicsDevice::default();
        let mut cache = TextureResourceCache::new();
        let first = crate::render::ImageUpdate::new(
            image,
            crate::render::ImageRevision::new(1),
            1,
            1,
            vec![1, 2, 3, 255],
        )
        .unwrap();
        cache.upload_updates(&mut device, &[first]).unwrap();
        let old_handle = cache.handle(image).unwrap();
        device.fail_next_submission = true;
        let resized = crate::render::ImageUpdate::new(
            image,
            crate::render::ImageRevision::new(2),
            2,
            1,
            vec![4, 5, 6, 255, 7, 8, 9, 255],
        )
        .unwrap();

        assert_eq!(
            cache.upload_updates(&mut device, &[resized]),
            Err(DeviceError::UnsupportedCommand)
        );
        assert_eq!(cache.handle(image), Some(old_handle));
        assert!(device.textures.contains_key(&old_handle));
        assert_eq!(device.textures.len(), 1);
    }
}
use std::collections::HashMap;

use super::{ImageId, ImageRevision, ImageUpdate};
