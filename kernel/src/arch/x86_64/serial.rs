use core::fmt;
use spin::Mutex;
use cpuio;

/// A COM serial port.
pub struct ComPort {
	/// COM ports are identified by the base address of their associated
	/// I/O registers.
	base_addr: u16
}

impl ComPort {
	/// Create a new COM port with the specified base address.
	const fn new(base_addr: u16) -> ComPort {
		ComPort { base_addr: base_addr }
	}
}

impl fmt::Write for ComPort {
	/// Output a string to our COM port.  This allows using nice,
	/// high-level tools like Rust's `write!` macro.
	fn write_str(&mut self, s: &str) -> fmt::Result {
		unsafe {
			let mut uart_port : cpuio::Port<u8> = cpuio::Port::new(self.base_addr);

			// Output each byte of our string.
			for &b in s.as_bytes() {
				// Write our byte.
				uart_port.write(b);
			}
		}
		Ok(())
	}
}

/// Our primary serial port.
pub static COM1: Mutex<ComPort> = Mutex::new(ComPort::new(0x3F8));
