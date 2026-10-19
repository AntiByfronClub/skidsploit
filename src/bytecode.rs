extern "C" {
    fn spPrepare(
        source: *const u8,
        sourceLength: i32,
        wrapFunction: i32,
        out: *mut u8,
        outCapacity: i32,
        outLength: *mut i32,
        error: *mut u8,
        errorCapacity: i32,
        errorLength: *mut i32,
    ) -> i32;
}

const OUTPUT_CAPACITY: usize = 16 * 1024 * 1024;
const ERROR_CAPACITY: usize = 1024;

pub fn prepare(source: &str, wrapFunction: bool) -> Result<Vec<u8>, String> {
    let sourceBytes = source.as_bytes();
    let mut out = vec![0u8; OUTPUT_CAPACITY];
    let mut outLength: i32 = 0;
    let mut error = vec![0u8; ERROR_CAPACITY];
    let mut errorLength: i32 = 0;

    let status = unsafe {
        spPrepare(
            sourceBytes.as_ptr(),
            sourceBytes.len() as i32,
            wrapFunction as i32,
            out.as_mut_ptr(),
            out.len() as i32,
            &mut outLength,
            error.as_mut_ptr(),
            error.len() as i32,
            &mut errorLength,
        )
    };

    if status == 0 && outLength > 0 {
        out.truncate(outLength as usize);
        return Ok(out);
    }

    error.truncate(errorLength.clamp(0, ERROR_CAPACITY as i32) as usize);
    let message = String::from_utf8_lossy(&error).trim().to_string();
    Err(if message.is_empty() {
        format!("prepare failed (status {})", status)
    } else {
        message
    })
}
