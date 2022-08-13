# Prometheus Exporter 
### Prometheus Rust exporters for token balances and accounts across a various networks

Thanks to [MindFlavor/prometheus_exporter](https://github.com/MindFlavor/prometheus_exporter) for the base exporter 

### Supported Networks 
- [x] Solana
- [ ] EVM

### Running 
Since you might only want to run an exporter for a single network each network has been broken into an example in the `./projects` directory. Simply run the network that you would like by using `--example <network>`

```
# first define the addresses you would like to watch in `main()` of `main.rs`
// Define addresses here 
let addresses = vec![
    "<address>",
];
```

```rust
cargo run --example solana --features="hyper_server"
```