use ring::aead::{open_in_place, seal_in_place, AES_256_GCM, OpeningKey, SealingKey};

pub fn encrypt_frame(data: &mut Vec<u8>, key: &[u8], iv: &[u8]) {
	let key = SealingKey::new(&AES_256_GCM, key).unwrap();
	let tag_size = key.algorithm().tag_len();
	let output_length = data.len() + tag_size;
	data.resize(output_length, 0);
	let result_length = seal_in_place(&key, iv, b"", data, tag_size).unwrap();
	assert!(data.len() == result_length);
}

pub fn decrypt_frame(data: &mut [u8], key: &[u8], iv: &[u8]) -> usize {
	let key = OpeningKey::new(&AES_256_GCM, key).unwrap();
	open_in_place(&key, iv, b"", 0, &mut data[..]).unwrap().len()
}
