use std::{fmt::Write, fs::File, io::{self, Read}, path::Path};

use sha2::{Sha256, Digest};



pub fn compute_file_hash(file_path: &Path) -> Result<String, io::Error> {
    let mut file_content: Vec<u8> = Vec::with_capacity(1024);
    let mut file = File::open(file_path)?;
    file.read_to_end(&mut file_content)?;

    compute_hash_string(&file_content)
}

fn compute_hash_string(data: &[u8]) -> Result<String, io::Error> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut result_string = String::with_capacity(80);
    result_string.push_str("sha256:");
    for b in result {
        write!(&mut result_string, "{:x}", b).map_err(|e| io::Error::other(e))?;
    }
    Ok(result_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_string() {
        // This test case was created using the existing output of compute_hash_string.  It won't
        // validate the correctness of the hash algorithm, but it will detect any changes in the
        // hash result over time

        let input_data = String::from("this is a test");
        let hash = compute_hash_string(input_data.as_bytes()).unwrap();
        assert_eq!(hash, "sha256:2e99758548972a8e8822ad47fa1017ff72f06f3ff6a016851f45c398732bc5c");
    }
}
