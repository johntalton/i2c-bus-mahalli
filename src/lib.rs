#![deny(clippy::all)]

use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc, Mutex,
};

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[cfg(target_os = "linux")]
use i2cdev::{
  core::{I2CDevice, I2CMessage, I2CTransfer, I2CMessage},
  linux::{LinuxI2CDevice, LinuxI2CError, LinuxI2CMessage},
};

#[napi]
pub const MAX_TRANSFER_LEN: u32 = 32;

#[napi]
pub const MIN_ADDR: u16 = 0x03;

#[napi]
pub const MAX_ADDR: u16 = 0x77;

#[napi(object)]
pub struct ScanOptions {
  pub start_addr: Option<u16>,
  pub end_addr: Option<u16>,
}

#[napi(object)]
pub struct MahalliReadResult<'env> {
  pub bytes_read: u32,
  pub buffer: ArrayBuffer<'env>,
}

#[napi(object)]
pub struct MahalliWriteResult {
  pub bytes_written: u32,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum I2cOp {
  RawRead { address: u16, length: usize },
  RawWrite { address: u16, payload: Vec<u8> },
  BlockRead { address: u16, register: u8, length: usize },
  BlockWrite { address: u16, register: u8, payload: Vec<u8> },
  Scan { start_addr: u16, end_addr: u16 },
}

pub enum I2cOutput {
  Read(u32, Vec<u8>),
  Write(u32),
  Scan(Vec<u32>),
}

#[cfg(target_os = "linux")]
type DeviceSlot = Mutex<Option<LinuxI2CDevice>>;
#[cfg(not(target_os = "linux"))]
type DeviceSlot = Mutex<Option<()>>;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct BusState {
  device: DeviceSlot,
  closed: AtomicBool,
}

fn check_addr(addr: u16) -> Result<()> {
  if !(MIN_ADDR..=MAX_ADDR).contains(&addr) {
    return Err(Error::new(
      Status::InvalidArg,
      format!("Address 0x{addr:02x} outside 0x{MIN_ADDR:02x}..=0x{MAX_ADDR:02x}"),
    ));
  }
  Ok(())
}

fn check_len(len: usize, label: &str) -> Result<()> {
  let max = MAX_TRANSFER_LEN as usize;
  if !(1..=max).contains(&len) {
    return Err(Error::new(
      Status::InvalidArg,
      format!("{label} length must be 1..={max} (got {len})"),
    ));
  }
  Ok(())
}

#[cfg(target_os = "linux")]
fn i2c_err(ctx: &str, err: LinuxI2CError) -> Error {
  Error::new(Status::GenericFailure, format!("[{err:?}] {ctx}: {err}"))
}

#[cfg(target_os = "linux")]
fn select_slave(dev: &mut LinuxI2CDevice, addr: u16) -> Result<()> {
  dev
    .set_slave_address(addr)
    .map_err(|e| i2c_err(&format!("set_slave_address(0x{addr:02x})"), e))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct I2cTask {
  state: Arc<BusState>,
  op: I2cOp,
}

#[napi]
impl Task for I2cTask {
  type Output = I2cOutput;
  type JsValue = Either3<MahalliReadResult<'static>, MahalliWriteResult, Vec<u32>>;

  fn compute(&mut self) -> Result<Self::Output> {
    if self.state.closed.load(Ordering::SeqCst) {
      return Err(Error::from_reason("I2C bus is closed"));
    }

    #[cfg(target_os = "linux")]
    {
      let mut guard = self
        .state
        .device
        .lock()
        .map_err(|_| Error::from_reason("I2C bus mutex poisoned"))?;
      if self.state.closed.load(Ordering::SeqCst) {
        *guard = None;
        return Err(Error::from_reason("I2C bus is closed"));
      }
      let dev = guard
        .as_mut()
        .ok_or_else(|| Error::from_reason("I2C bus is closed"))?;

      let res = match &self.op {
        I2cOp::RawRead { address, length } => {
          select_slave(dev, *address)?;
          let mut buf = vec![0u8; *length];
          dev
            .transfer(&mut [LinuxI2CMessage::read(&mut buf)])
            .map_err(|e| i2c_err(&format!("i2cRead(0x{address:02x}, len={length})"), e))?;
          Ok(I2cOutput::Read(*length as u32, buf))
        }
        I2cOp::RawWrite { address, payload } => {
          select_slave(dev, *address)?;
          dev
            .transfer(&mut [LinuxI2CMessage::write(payload)])
            .map_err(|e| i2c_err(&format!("i2cWrite(0x{address:02x}, len={})", payload.len()), e))?;
          Ok(I2cOutput::Write(payload.len() as u32))
        }
        I2cOp::BlockRead { address, register, length } => {
          select_slave(dev, *address)?;
          let mut data = dev
            .smbus_read_i2c_block_data(*register, *length as u8)
            .map_err(|e| i2c_err(&format!("readI2cBlock(0x{address:02x}, reg=0x{register:02x})"), e))?;
          data.truncate(*length);
          Ok(I2cOutput::Read(data.len() as u32, data))
        }
        I2cOp::BlockWrite { address, register, payload } => {
          select_slave(dev, *address)?;
          dev
            .smbus_write_i2c_block_data(*register, payload)
            .map_err(|e| i2c_err(&format!("writeI2cBlock(0x{address:02x}, reg=0x{register:02x})"), e))?;
          Ok(I2cOutput::Write(payload.len() as u32))
        }
        I2cOp::Scan { start_addr, end_addr } => {
          let mut found = Vec::new();
          for addr in *start_addr..=*end_addr {
            if self.state.closed.load(Ordering::SeqCst) {
              return Err(Error::from_reason("I2C bus closed during scan"));
            }
            select_slave(dev, addr)?;
            let probe = match addr {
              0x30..=0x37 | 0x50..=0x5f => dev.smbus_read_byte().map(|_| ()),
              _ => dev.smbus_write_quick(false),
            };
            match probe {
              Ok(()) => found.push(addr as u32),
              Err(LinuxI2CError::Errno(_)) => {}
              Err(e) => return Err(i2c_err(&format!("scan(0x{addr:02x})"), e)),
            }
          }
          Ok(I2cOutput::Scan(found))
        }
      };

      if self.state.closed.load(Ordering::SeqCst) {
        *guard = None;
      }
      res
    }

    #[cfg(not(target_os = "linux"))]
    Err(Error::from_reason("i2cdev is only supported on Linux"))
  }

  fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
    match output {
      I2cOutput::Read(bytes_read, data) => Ok(Either3::A(MahalliReadResult {
        bytes_read,
        buffer: ArrayBuffer::from_data(&env, data)?,
      })),
      I2cOutput::Write(bytes_written) => Ok(Either3::B(MahalliWriteResult { bytes_written })),
      I2cOutput::Scan(addrs) => Ok(Either3::C(addrs)),
    }
  }
}

#[napi]
pub struct Mahalli {
  state: Arc<BusState>,
}

impl Mahalli {
  fn task(&self, op: I2cOp) -> Result<AsyncTask<I2cTask>> {
    Ok(AsyncTask::new(I2cTask {
      state: Arc::clone(&self.state),
      op,
    }))
  }
}

#[napi]
impl Mahalli {
  #[napi(constructor)]
  pub fn new(bus_path: String) -> Result<Self> {
    #[cfg(target_os = "linux")]
    {
      let dev = LinuxI2CDevice::new(&bus_path, 0x00)
        .map_err(|e| i2c_err(&format!("Failed to open {bus_path}"), e))?;
      Ok(Self {
        state: Arc::new(BusState {
          device: Mutex::new(Some(dev)),
          closed: AtomicBool::new(false),
        }),
      })
    }
    #[cfg(not(target_os = "linux"))]
    {
      let _ = bus_path;
      Err(Error::from_reason("i2cdev is only supported on Linux"))
    }
  }

  #[napi(getter)]
  pub fn is_closed(&self) -> bool {
    self.state.closed.load(Ordering::SeqCst)
  }

  #[napi]
  pub fn close(&self) {
    self.state.closed.store(true, Ordering::SeqCst);
    if let Ok(mut guard) = self.state.device.try_lock() {
      *guard = None;
    }
  }

  #[napi(js_name = "[Symbol.dispose]")]
  pub fn dispose(&self) {
    self.close();
  }

  #[napi(ts_return_type = "Promise<MahalliReadResult>")]
  pub fn i2c_read(&self, address: u16, length: u32) -> Result<AsyncTask<I2cTask>> {
    check_addr(address)?;
    check_len(length as usize, "i2cRead")?;
    self.task(I2cOp::RawRead {
      address,
      length: length as usize,
    })
  }

  #[napi(ts_return_type = "Promise<MahalliWriteResult>")]
  pub fn i2c_write(
    &self,
    address: u16,
    #[napi(ts_arg_type = "ArrayBufferView<ArrayBuffer>")] payload: BufferSlice,
  ) -> Result<AsyncTask<I2cTask>> {
    check_addr(address)?;
    check_len(payload.len(), "i2cWrite")?;
    self.task(I2cOp::RawWrite {
      address,
      payload: payload.to_vec(),
    })
  }

  #[napi(ts_return_type = "Promise<MahalliReadResult>")]
  pub fn read_i2c_block(&self, address: u16, register: u8, length: u8) -> Result<AsyncTask<I2cTask>> {
    check_addr(address)?;
    check_len(length as usize, "readI2cBlock")?;
    self.task(I2cOp::BlockRead {
      address,
      register,
      length: length as usize,
    })
  }

  #[napi(ts_return_type = "Promise<MahalliWriteResult>")]
  pub fn write_i2c_block(
    &self,
    address: u16,
    register: u8,
    #[napi(ts_arg_type = "ArrayBufferView<ArrayBuffer>")] payload: BufferSlice,
  ) -> Result<AsyncTask<I2cTask>> {
    check_addr(address)?;
    check_len(payload.len(), "writeI2cBlock")?;
    self.task(I2cOp::BlockWrite {
      address,
      register,
      payload: payload.to_vec(),
    })
  }

  #[napi(ts_return_type = "Promise<number[]>")]
  pub fn scan(&self, options: Option<ScanOptions>) -> Result<AsyncTask<I2cTask>> {
    let (start_addr, end_addr) = options
      .map(|o| (o.start_addr.unwrap_or(MIN_ADDR), o.end_addr.unwrap_or(MAX_ADDR)))
      .unwrap_or((MIN_ADDR, MAX_ADDR));
    check_addr(start_addr)?;
    check_addr(end_addr)?;
    if start_addr > end_addr {
      return Err(Error::new(
        Status::InvalidArg,
        format!("startAddr (0x{start_addr:02x}) > endAddr (0x{end_addr:02x})"),
      ));
    }
    self.task(I2cOp::Scan { start_addr, end_addr })
  }
}
