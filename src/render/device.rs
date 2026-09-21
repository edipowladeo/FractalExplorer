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
}

pub trait GraphicsDevice: Send {
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
        let mut uploaded = 0;
        for update in updates {
            let dimensions = update.dimensions();
            if self
                .textures
                .get(&update.image())
                .is_some_and(|cached| cached.revision.value() >= update.revision().value())
            {
                continue;
            }

            let handle = match self.textures.get(&update.image()) {
                Some(cached) if cached.dimensions == dimensions => cached.handle,
                Some(cached) => {
                    device.destroy_texture(cached.handle);
                    device.create_texture(TextureDescriptor {
                        width: dimensions.0,
                        height: dimensions.1,
                        format: TextureFormat::Rgba8,
                    })?
                }
                None => device.create_texture(TextureDescriptor {
                    width: dimensions.0,
                    height: dimensions.1,
                    format: TextureFormat::Rgba8,
                })?,
            };
            commands.write_texture(handle, dimensions.0, dimensions.1, update.rgba8().to_vec());
            self.textures.insert(
                update.image(),
                CachedTexture {
                    handle,
                    revision: update.revision(),
                    dimensions,
                },
            );
            uploaded += 1;
        }
        if uploaded > 0 {
            device.submit(commands)?;
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
mod tests {
    use super::{
        BufferDescriptor, BufferHandle, BufferUsage, Command, CommandList, DeviceError,
        GraphicsDevice, TextureDescriptor, TextureFormat, TextureHandle, TextureResourceCache,
    };

    #[derive(Default)]
    struct RecordingDevice {
        next_handle: u64,
        submitted: Vec<CommandList>,
    }

    impl GraphicsDevice for RecordingDevice {
        fn create_buffer(
            &mut self,
            _descriptor: BufferDescriptor,
        ) -> Result<BufferHandle, DeviceError> {
            self.next_handle += 1;
            Ok(BufferHandle::new(self.next_handle))
        }

        fn create_texture(
            &mut self,
            _descriptor: TextureDescriptor,
        ) -> Result<TextureHandle, DeviceError> {
            self.next_handle += 1;
            Ok(TextureHandle::new(self.next_handle))
        }

        fn submit(&mut self, commands: CommandList) -> Result<(), DeviceError> {
            self.submitted.push(commands);
            Ok(())
        }

        fn destroy_buffer(&mut self, _buffer: BufferHandle) {}

        fn destroy_texture(&mut self, _texture: TextureHandle) {}
    }

    #[test]
    fn device_contract_keeps_resource_creation_and_submission_backend_independent() {
        let mut device = RecordingDevice::default();
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
        assert_eq!(device.submitted.len(), 1);
        assert!(matches!(
            &device.submitted[0].commands()[0],
            Command::WriteBuffer { offset: 4, .. }
        ));
    }

    #[test]
    fn texture_cache_uploads_only_new_revisions_and_reuses_same_dimensions() {
        let mut device = RecordingDevice::default();
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
        assert_eq!(device.submitted.len(), 2);
    }
}
use std::collections::HashMap;

use super::{ImageId, ImageRevision, ImageUpdate};
