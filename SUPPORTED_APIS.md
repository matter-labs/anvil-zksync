# 🔧 Supported APIs for In-Memory Node 🔧

> ⚠️ **WORK IN PROGRESS**: This list is non-comprehensive and being updated

# Key

The `status` options are:

+ `SUPPORTED` - Basic support is complete
+ `PARTIALLY` - Partial support and a description including more specific details
+ `NOT IMPLEMENTED` - Currently not supported/implemented

# `CONFIG NAMESPACE`

## `config_getShowCalls`

[source](src/configuration_api.rs)

Gets the current value of `show_calls` that's originally set with `--show-calls` option

### Arguments

+ _NONE_

### Status

`SUPPORTED`

### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_getShowCalls","params": []}'
```

## `config_setShowCalls`

[source](src/configuration_api.rs)

Updates `show_calls` to print more detailed call traces

### Arguments

+ `value: String ('None', 'User', 'System', 'All')`

### Status

`SUPPORTED`

### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowCalls","params": ["all"]}'
```

## `config_setResolveHashes`

[source](src/configuration_api.rs)

Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces

### Arguments

+ `value: boolean`

### Status

`SUPPORTED`

### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setResolveHashes","params": [true]}'
```
