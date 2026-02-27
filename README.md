Extracts all UniswapV2 and UniswapV3 pools and their tokens.

Additional option: lookup the price data of the last x blocks.

To run the code you will need a connection to an Ethereum node.
Create an .env file and fill in the HTTPS_URL of your node:

.env
```
HTTPS_URL="https://eth-mainnet.public.blastapi.io"
```

If you want logging add RUST_LOG=INFO to the .env file.

Goal: Make an extractable registry of all possible erc20-tradable tokens on the
Ethereum chain.
