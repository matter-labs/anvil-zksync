# What is zksync-error?

A generated crate describing all errors that are:

- originating in one of ZKsync components;
- visible outside the component, either to users, or to other components.

`anvil-zksync` is one of such components.

The motivation behind `zksync-error` is to:
- improve developer and user experience by agreeing on a single source of truth
  for source code error descriptions in Rust and Typescript, their
  documentation, format and error messages, pretty-printing etc
- make a unified failure description system but retain the ability to work on
  parts of different ZKsync components separately;
- provide an ease of usage and ease of migration for developers;
- agree on a single documentation standard for public facing errors;
- provide ZKsync components with access to the documentation in runtime so that
  they could react to failures and guide users towards solutions.

Every component has and will have their own ways of handling failures;
zksync-error is only concerned with providing an abstraction layer for outwards
facing errors.

# Error hierarchy

- Errors are described as pure data in JSON files. This fragment of a JSON file
  describes an individual error with a field `msg` of type `string`:

```json
 {
    "name": "FailedToAppendTransactionToL2Block",
    "code": 17,
    "message": "Failed to append the transaction to the current L2 block: {msg}",
    "fields": [
        {
            "name": "msg",
            "type": "string"
        }
    ]
}
```

- Every error has a *code*, belongs to a *component*, and every component belongs to a *domain*.
  This structure matches the structure of JSON files. For example, this file
  describes one domain `Anvil` with one component `AnvilEnvironment` and one
  error `LogFileAccessError`:

```js
{
    "types": [],
    "domains": [
        {
            "domain_name": "Anvil",
            "domain_code": 5,
            "identifier_encoding": "anvil",
            "components" : [
                {
                    "component_name": "AnvilEnvironment",
                    "component_code": 1,
                    "identifier_encoding": "env",
                    "errors" : [
                        {
                            "name": "LogFileAccessError",
                            "code": 1,
                            "message": "Unable to access log file: {log_filename}",
                            "fields": [
                                {
                                    "name": "log_filename",
                                    "type": "string"
                                },
                                {
                                    "name": "wrapped_error",
                                    "type": "string"
                                }
                            ]
                       }
                    ]
                }
            ]
        }
        ]
}
```

- The error codes should be unique inside a single component.
- Domains and components have identifiers -- short strings provided by
  `identifier_encoding` field in JSON files. For example, the domain `Anvil` in
  the example above has an identifier `anvil`.

- Errors have identifiers in form
  `[<domain_identifier>-<component_identifier>-<error_code>]`. For example, the
  error `LogFileAccessError` in the example above has an identifier
  `[anvil-env-1]`. Identifiers are globally unique and never reused if the error is deprecated.
  
- The main JSON file is located in [zksync-error
  repository](https://github.com/matter-labs/zksync-error/blob/main/zksync-root.json).
  It is linked to similar files in other repositories through `takeFrom` fields.
  These files are automatically merged to produce a single tree of
  domains-components-errors.
  This tree-like model is then used to generate:
    + `zksync-error` crate, defining errors for Rust code;
    + documentation for errors in MDbook format.
    + in future, TypeScript code to interact with these errors.

This approach allows to define errors in the projects where they are used, for
example in Solidity compiler or VM.

# Adding new errors

Put new errors in the file [etc/errors/anvil.json](/etc/errors/anvil.json).

Every component implicitly contains an error with code 0 meaning an umbrella
"generic error". You usually should not define this error yourself.

```js
{
        "name": "GenericError",
        "code": 0,
        "message": "Generic error: {message}",
        "fields": [
          {
            "name": "message",
            "type": "string"
          }
        ]
      }
```

You may define other errors, for example:

```
{
    "types": [],
    "domains": [
        {
            "domain_name": "Anvil",
            "domain_code": 5,
            "identifier_encoding": "anvil",
            "components" : [
            {
                "component_name": "StateLoader",
                "component_code": 2,
                "identifier_encoding": "state",
                "errors" : [
                    {
                        "name": "LoadingStateOverExistingStateError",
                        "code": 1,
                        "message": "Loading state into a node with existing state is not allowed.",
                        "doc" : {
                            "summary": "It is not allowed to load a state overriding the existing node state.",
                            "description": "It is not allowed to load a state overriding the existing node state. If you have a use case for that, please create an issue."
                            }
                    }
                ]
            }
        ]
    }
    ]
}
```

## Publishing

After you change `anvil.json`, rebuild `anvil-zksync` and `zksync-error` crate
will be regenerated.
You may also build `zksync-error` crate using CLI.

You may use `cargo build -vv` to see the debug output of the code generator.
New errors will appearing in [definitions.rs](crates/zksync_error/src/error/definitions.rs).

When you publish your `anvil.json` to the repository, other projects will see
the updated definitions. Then they should regenerate their `zksync-error` crates
to use them. 

## Error fields
By default, the fields of errors can have one of the following types:

- string
- int / uint
- `wrapped_error`: a JSON value serialized through `serde_json`.
- `map`: a string to string mapping
- `bytes`: a sequence of bytes
- type of another error, defined in one of JSONs.

Errors in `zksync-error` are part of a component's interface, so they can not
have types that are unknown to other components.
This prevents directly wrapping errors that are defined internally in one of components.

# Accessing new errors

In Rust, use the interface `zksync_error::<domain_identifier>::<component_identifier>::<error_name>.
 
For example, suppose we have defined a domain `Anvil` with two components `Environment` and `Generic`. Their identifiers
The following example outlines the structure of the interface, in Rust pseudocode:


```rust
pub mod anvil {
    type AnvilError;
    pub mod env {
        type AnvilEnvironmentError;
        type GenericError;
        type FirstCustomError;
        macro generic_error;
        // converts anything that implements `to_string` into a generic error inside this component.
        fn to_generic<E: Display>(error); 
    }
    pub mod gen {
    ...
    }
}
```




# Migration

The root type of the error hierarchy is `ZksyncError`. For example, the error
`GenericError` from the domain `Anvil` and its component `AnvilEnvironment`
is instantiated as follows:

```rust
    ZksyncError::Anvil(// Domain
        Anvil::AnvilEnvironment ( // Component
            AnvilEnvironment::GenericError { // Error
                message: "hello!".to_string()
            })
    );
```

**You don't need to construct this instance explicitly**. There are adapters
implementing `Into` for every use case:

1. from errors to domain errors types e.g. `Anvil`
2. from errors to component errors types e.g. `AnvilEnvironment`
3. from errors to the root type `ZksyncError`
4. from domain errors to `ZksyncError`
5. from `anyhow::Error` to component errors.

This facilitates the migration.

```rust
// This works:
let err : ZksyncError = AnvilEnvironment::GenericError {
                message: "hello!".to_string()
            }.into();


// instead of passing type to `into` where type derivation does not work
let err_same = AnvilEnvironment::GenericError {
                message: "hello!".to_string()
            }.to_unified();
            
// Also works:
fn test() -> Result<(), ZksyncError> {
    Err(AnvilEnvironmentError::GenericError {
        message: "Oops".to_string(),
    })?;
    Ok(())
}

// Also works:
fn test_domain() -> Result<(), AnvilError> {
    Err(AnvilEnvironmentError::GenericError {
        message: "Oops".to_string(),
    })?;
    Ok(())
}

// anyhow -> component, produces the default GenericError belonging to this component
fn test_anyhow() -> Result<(), AnvilEnvironmentError> {
    Err(anyhow::anyhow!("Oops"))?;
    Ok(())
}

// 
fn test_domain() -> Result<(), AnvilError> {
    Err(AnvilEnvironmentError::GenericError {
        message: "Oops".to_string(),
    })?;
    Ok(())
}
```






## If the function returns `Result<_,anyhow::Error>`

Suppose you want to throw an error from a component `AnvilEnvironment` of domain `Anvil` instead of `anyhow::Error`. Follow these steps:

1. Change the return type of the function to `Result<_,
   zksync_error::anvil::gen::GenericError>` and it will be automatically cast to
   a `GenericError` of this component.

   On error throwing sites, if the implicit call to `into` is not sufficient or is not applicable, then try to do the following:
   
   - map the error using the function `zksync_error::anvil::gen::to_generic`,
   for example:
   
   
```rust
//replace the first line with the second
    let log_file = File::create(&config.log_file_path)?;
    // Instead of returning `std::io::Error` you now return `anvil::gen::GenericError` containing it
    let log_file = File::create(&config.log_file_path).map_err(to_generic)?;

//This is equivalent to `return anyhow!(error)`
anyhow::bail!(
        "fork is using unsupported fee parameters: {:?}",
        fork_client.details.fee_params
    )

// We still return `anyhow::Error` but it is cast to `GenericError` of our component
return Err(anyhow!(
    "fork is using unsupported fee parameters: {:?}",
    fork_client.details.fee_params
)
.into())

// Instead of `into` we may use a macro `generic_error` imported from `anvil::gen::generic_error`, or from a namespace of a different component.
return Err(generic_error!(
    "fork is using unsupported fee parameters: {:?}",
    fork_client.details.fee_params
))
```
  
2. Introduce more errors, corresponding to different failure situations.

   Describe the new error in the JSON file, and replace it on the throw site.


   
## If the function returns `Result` with an internal error 

Many functions return `Result<_, CustomErrorType>` and if the error
`CustomErrorType` ends up getting outside the component, you may want to modify
the function to return 
`Result<_, zksync_error::anvil::env::AnvilEnvironmentError>` instead,  or an
error related to a different component. 

Suppose functions `caller1`, `caller2` and so on call a function 
`fn f()->Result<_, CustomErrorType>`, and we want to migrate `f` to 
`AnvilEnvironmentError` step by step.

1. Assess if you *really* need to rewrite it. Remember that all error types
   described in JSON and provided by `zksync_error` are for public errors only!
   If this error gets through the call stack to the component user, then it
   makes sense to migrate it to `ZksyncError`.
2. On the call site, in `caller`, map the error value using
   `map_err(to_generic)` and refactor the `caller` until it compiles.
3. Make a copy of `f`, say, `f_new`.
4. Replace the return type in `f_new` with `Result<_, AnvilEnvironmentError>`. 
   Whenever `f_new` returns `Err(e:CustomErrorType)`,  it may return `anvil::env::GenericError` instead.
   Use `.map_err(to_generic)`, explicit or implicit `into`, and `anvil::env::generic_error!` macros.
5. Work through the callers, one at a time. Pick a caller, say, `caller1`.
   Replace calls to `f` with calls to `f_new` and remove unused conversions. 
6. Repeat until all callers are refactored.
7. Remove the function `f` and rename `f_new` back to `f`.
   




## Functions returning errors from multiple components

If a function may return errors that are spread among components (e.g. either
`AnvilEnvironment` or `AnvilStateLoader` components) then we advise to return a
type `AnvilError` from it. 
Such function are rare and require more manual work, but the algorithm is the
same -- just use `to_domain` helper along with `generic_error!` macro.


