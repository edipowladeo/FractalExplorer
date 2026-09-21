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

    pub fn write_texture(&mut self, texture: TextureHandle, bytes: Vec<u8>) {
        self.commands.push(Command::WriteTexture { texture, bytes });
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

#[cfg(test)]
mod tests {
    use super::{
        BufferDescriptor, BufferHandle, BufferUsage, Command, CommandList, DeviceError,
        GraphicsDevice, TextureDescriptor, TextureFormat, TextureHandle,
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
        commands.write_texture(texture, vec![255; 16]);
        device.submit(commands).unwrap();

        assert_eq!(buffer.value(), 1);
        assert_eq!(texture.value(), 2);
        assert_eq!(device.submitted.len(), 1);
        assert!(matches!(
            &device.submitted[0].commands()[0],
            Command::WriteBuffer { offset: 4, .. }
        ));
    }
}
