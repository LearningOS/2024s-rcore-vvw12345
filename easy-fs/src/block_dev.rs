use core::any::Any;
/// Trait for block devices
/// which reads and writes data in the unit of blocks

/// 块设备接口层
/// 以块为大小单位读写磁盘块设备
/// 没有具体的实现 由具体的块设备驱动程序实现
pub trait BlockDevice: Send + Sync + Any {
    ///Read data form block to buffer
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    ///Write data from buffer to block
    fn write_block(&self, block_id: usize, buf: &[u8]);
}
