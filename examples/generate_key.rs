use elliptic_curve::SecretKey;
use pkcs8::EncodePrivateKey;
use rand::rngs::ThreadRng;

fn main() {
    let secret_key = SecretKey::<p256::NistP256>::random(&mut ThreadRng::default());
    let pem = secret_key
        .to_pkcs8_pem(Default::default())
        .expect("failed to encode key");
    println!("{}", pem.as_str());
}
