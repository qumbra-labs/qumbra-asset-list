//! `asset-list` — the Qumbra asset list's three operations.
//!
//! ```text
//! asset-list keygen --seed-out FILE --pub-out FILE   # offline, once
//! asset-list sign   --seed FILE --list lists/<genesis>.json
//! asset-list verify --pub FILE --list lists/<genesis>.json
//! ```
//!
//! The signature is ML-DSA-65 over `qumbra:asset-list:v1\0 ‖ list bytes`
//! (`qumbra_wallet::asset_view::ASSET_LIST_DOMAIN`), written beside the list
//! as `<list>.sig`. `sign` re-verifies what it wrote with the wallet's own
//! `verify_asset_list` before it returns, so a list that a wallet would
//! refuse is never left signed.
//!
//! The seed file is the list's signing key. It never belongs in this
//! repository, on a server, or in a CI secret: keep it offline.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ml_dsa::{Keypair, MlDsa65, Signer, SigningKey, B32};
use qumbra_wallet::asset_view::{verify_asset_list, ListKey, ASSET_LIST_DOMAIN};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("the seed file must hold 64 hex characters".into());
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn flag(args: &[String], name: &str) -> Result<PathBuf, String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| PathBuf::from(&w[1]))
        .ok_or_else(|| format!("missing {name}"))
}

fn signing_key(seed_path: &Path) -> Result<SigningKey<MlDsa65>, String> {
    let seed = unhex32(&fs::read_to_string(seed_path).map_err(|e| format!("{}: {e}", seed_path.display()))?)?;
    let seed: B32 = seed.into();
    Ok(SigningKey::<MlDsa65>::from_seed(&seed))
}

fn keygen(args: &[String]) -> Result<(), String> {
    let (seed_out, pub_out) = (flag(args, "--seed-out")?, flag(args, "--pub-out")?);
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no OS randomness: {e}"))?;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&seed_out).map_err(|e| format!("{} (never overwritten): {e}", seed_out.display()))?;
    f.write_all(hex(&seed).as_bytes()).map_err(|e| e.to_string())?;
    let key = SigningKey::<MlDsa65>::from_seed(&seed.into());
    let vk = key.verifying_key().encode();
    fs::write(&pub_out, vk.as_slice()).map_err(|e| format!("{}: {e}", pub_out.display()))?;
    let fp = ListKey::from_encoded(vk.as_slice()).expect("a derived key encodes").fingerprint();
    println!("seed (SECRET, keep offline): {}", seed_out.display());
    println!("public key ({} B): {}", vk.as_slice().len(), pub_out.display());
    println!("fingerprint: {}", hex(&fp));
    Ok(())
}

fn sign(args: &[String]) -> Result<(), String> {
    let (seed, list) = (flag(args, "--seed")?, flag(args, "--list")?);
    let bytes = fs::read(&list).map_err(|e| format!("{}: {e}", list.display()))?;
    let key = signing_key(&seed)?;
    let mut msg = ASSET_LIST_DOMAIN.to_vec();
    msg.extend_from_slice(&bytes);
    let sig: ml_dsa::Signature<MlDsa65> = key.sign(&msg);
    let sig = sig.encode().as_slice().to_vec();
    let vk = ListKey::from_encoded(key.verifying_key().encode().as_slice()).expect("a derived key encodes");
    let parsed = verify_asset_list(&bytes, &sig, &vk).map_err(|e| format!("refusing to sign: {e}"))?;
    let out = PathBuf::from(format!("{}.sig", list.display()));
    fs::write(&out, &sig).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("signed {} ({} assets) -> {}", list.display(), parsed.assets.len(), out.display());
    println!("list digest:  {}", hex(&parsed.digest));
    println!("signer:       {}", hex(&vk.fingerprint()));
    Ok(())
}

fn verify(args: &[String]) -> Result<(), String> {
    let (pubf, list) = (flag(args, "--pub")?, flag(args, "--list")?);
    let vk = fs::read(&pubf).map_err(|e| format!("{}: {e}", pubf.display()))?;
    let vk = ListKey::from_encoded(&vk).ok_or("the public key file is not an encoded ML-DSA-65 key")?;
    let bytes = fs::read(&list).map_err(|e| format!("{}: {e}", list.display()))?;
    let sigf = PathBuf::from(format!("{}.sig", list.display()));
    let sig = fs::read(&sigf).map_err(|e| format!("{}: {e}", sigf.display()))?;
    let parsed = verify_asset_list(&bytes, &sig, &vk).map_err(|e| e.to_string())?;
    println!("OK — {} for genesis {}", parsed.network, hex(&parsed.genesis));
    println!("digest {} signer {}", hex(&parsed.digest), hex(&vk.fingerprint()));
    if parsed.testnet {
        println!("TEST NETWORK: every row under this list is test money");
    }
    for a in parsed.assets.values() {
        println!("  #{:<5} {:<10} {} decimals  {}", a.id, a.ticker, a.decimals, a.name);
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let r = match args.get(1).map(String::as_str) {
        Some("keygen") => keygen(&args),
        Some("sign") => sign(&args),
        Some("verify") => verify(&args),
        _ => Err("usage: asset-list keygen --seed-out F --pub-out F | sign --seed F --list L | verify --pub F --list L".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("asset-list: {e}");
            ExitCode::FAILURE
        }
    }
}
