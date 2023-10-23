# Usage

In-Memory node can be utilized for a variety of reasons. 


## Network Details
The `era_test_node` has the following default network configurations:

* **L2 RPC:** `http://localhost:8011`
* **Network Id:** 260
  
These can be configured to your preference.

> *Note:* Please note that the existing implementation does not facilitate communication with Layer 1. As a result, an L1 RPC is not available.

## Caching

The node will cache certain network request by default to disk in the `.cache` directory. Alternatively the caching can be disabled or set to in-memory only
via the `--cache=none|memory|disk` parameter. 

```bash
era_test_node --cache=none run
```

```bash
era_test_node --cache=memory run
```

Additionally when using `--cache=disk`, the cache directory may be specified via `--cache-dir` and the cache may
be reset on startup via `--reset-cache` parameters.
```bash
era_test_node --cache=disk --cache-dir=/tmp/foo --reset-cache run
```

## Pre-configured Rich Wallets

The node also includes pre-configured "rich" accounts for testing:

| Account Id    |  Private Key  |
| ------------- | ------------- |
|0x36615Cf349d7F6344891B1e7CA7C72883F5dc049 | 0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110 |
|0xa61464658AfeAf65CccaaFD3a512b69A83B77618 | 0xac1e735be8536c6534bb4f17f06f6afc73b2b5ba84ac2cfb12f7461b20c0bbe3 |
|0x0D43eB5B8a47bA8900d84AA36656c92024e9772e | 0xd293c684d884d56f8d6abd64fc76757d3664904e309a0645baf8522ab6366d9e |
|0xA13c10C0D5bd6f79041B9835c63f91de35A15883 | 0x850683b40d4a740aa6e745f889a6fdc8327be76e122f5aba645a5b02d0248db8 |
|0x8002cD98Cfb563492A6fB3E7C8243b7B9Ad4cc92 | 0xf12e28c0eb1ef4ff90478f6805b68d63737b7f33abfa091601140805da450d93 |
|0x4F9133D1d3F50011A6859807C837bdCB31Aaab13 | 0xe667e57a9b8aaa6709e51ff7d093f1c5b73b63f9987e4ab4aa9a5c699e024ee8 |
|0xbd29A1B981925B94eEc5c4F1125AF02a2Ec4d1cA | 0x28a574ab2de8a00364d5dd4b07c4f2f574ef7fcc2a86a197f65abaec836d1959 |
|0xedB6F5B4aab3dD95C7806Af42881FF12BE7e9daa | 0x74d8b3a188f7260f67698eb44da07397a298df5427df681ef68c45b34b61f998 |
|0xe706e60ab5Dc512C36A4646D719b889F398cbBcB | 0xbe79721778b48bcc679b78edac0ce48306a8578186ffcb9f2ee455ae6efeace1 |
|0xE90E12261CCb0F3F7976Ae611A29e84a6A85f424 | 0x3eb15da85647edd9a1159a4a13b9e7c56877c4eb33f614546d4db06a51868b1c |
