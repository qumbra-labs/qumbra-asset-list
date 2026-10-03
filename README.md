# qumbra-asset-list

> [中文](README-zh.md)

The signed list of Annulet assets that Qumbra wallets show by name.

On an Annulet chain an asset is a registry slot: a number from 1 to 65535. The registry leaf holds an `issuer_key`, a mode and policy roots. **It has no name, no ticker and no decimals.** This repository is where those come from (`qumbra-design` `wallet-assets-decision.md`, D1–D3).

## Rules a wallet applies

- **An asset is `(genesis hash, id)`, never a name.** There is one list per network: `lists/<genesis-hash>.json`. A wallet ignores a list for another genesis.
- **The list is signed.** The signature is ML-DSA-65 over `qumbra:asset-list:v1\0 ‖ the file's exact bytes`, stored beside the list as `<list>.json.sig`. The key's public half is `keys/list-key.pub`. A wallet build embeds the list and its signature from a pinned commit of this repository, and it verifies them against a compiled-in key.
- **The list pins the issuer key (D2).** A wallet shows the listed name only while the chain's registry leaf carries the listed `issuer_key`. If the issuer rotates its key, the wallet shows "issuer changed" and the raw balance until a new list is signed.
- **Unlisted assets are still shown**, as "asset #N" in raw base units. Their decimals are never guessed.
- **A `testnet: true` list** marks every row it covers as test money, on every surface — listed assets, unlisted ones and the fee unit alike. A list is for one network, and a network either is a testnet or is not.

## Format (v1)

```json
{
  "v": 1,
  "network": "<label>",
  "genesis": "<64 lowercase hex: keccak256 of the genesis file>",
  "testnet": true,
  "assets": [
    { "id": 1, "issuer_key": "<64 lowercase hex: the leaf's 4 lanes, little-endian>",
      "name": "<1–64 printable chars>", "ticker": "<1–12 of A-Z a-z 0-9 . ->",
      "decimals": 6 }
  ]
}
```

The list must be at most 1 MiB. The parser is strict:

- `v` is checked first, so a later version is refused as a version;
- unknown keys are refused;
- ids must ascend and must not repeat;
- id 0 (the fee unit) is never listed;
- `decimals` must be 18 or less;
- a ticker may not repeat within a list (case-insensitively).

What the parser does **not** judge is resemblance: a name or ticker that looks like another asset's (homoglyphs, or a "USDT" that is not Tether's). That is this repository's review job, before anything is signed.

The parser that enforces this is the wallet's own (`qumbra_wallet::asset_view::verify_asset_list`). The tool below uses the same parser, so a list it signs is a list every wallet accepts.

## Adding or changing an entry

1. Read the asset's registry leaf from a node: `GET /v1/registry/<id>` (bytes 49..81 are the `issuer_key`). Copy the hex **from that output**, never from memory.
2. Edit the list in a PR. Review the name, ticker and decimals against the issuer's own statement.
3. **The key holder signs offline** (below), commits `<list>.json.sig`, and CI verifies it.
4. Wallets pick up the change only when a release repins this repository's commit.

## The tool

```sh
cargo build --release --manifest-path tools/asset-list/Cargo.toml
T=tools/asset-list/target/release/asset-list

$T keygen --seed-out /path/offline/list.seed --pub-out keys/list-key.pub   # once, offline
$T sign   --seed /path/offline/list.seed --list lists/<genesis>.json        # writes .sig, re-verifies
$T verify --pub keys/list-key.pub --list lists/<genesis>.json
```

**The seed file is the signing key.** It never goes into this repository (`.gitignore` refuses `*.seed`), onto a server or into a CI secret. Keep it offline, the way the release signing key is kept.

## Lists

| Network | Genesis | Assets |
|---|---|---|
| annulet-gateway-testnet | `fcf7d570…b438` | 1 tUSDT, 1000 TUSD, 1001 TCLOAKED (all test money) |
