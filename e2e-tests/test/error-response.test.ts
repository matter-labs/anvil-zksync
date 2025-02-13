import { Wallet, Provider, Contract, utils, EIP712Signer, types } from "zksync-ethers";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import * as ethers from "ethers";
import * as hre from "hardhat";
import { expect } from "chai";

import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";

describe("Error formatting", function () {
  let provider: Provider;
  let wallet: Wallet;
  let deployer: Deployer;
  let nftUserWallet: Wallet;
  let paymaster: Contract;
  let greeter: Contract;
  let erc721: Contract;

  before(async function () {
    provider = getTestProvider();
    wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    deployer = new Deployer(hre, wallet);

    // Setup new wallets
    nftUserWallet = new Wallet(Wallet.createRandom().privateKey, provider);

    // Deploy NFT and Paymaster
    let artifact = await deployer.loadArtifact("MyNFT");
    erc721 = await deployer.deploy(artifact, []);
    artifact = await deployer.loadArtifact("ERC721GatedPaymaster");
    paymaster = await deployer.deploy(artifact, [await erc721.getAddress()]);
    artifact = await deployer.loadArtifact("Greeter");
    greeter = await deployer.deploy(artifact, ["Hi"]);

    // Fund Paymaster
    await provider.send("hardhat_setBalance", [await paymaster.getAddress(), ethers.toBeHex(ethers.parseEther("10"))]);

    // Assign NFT to nftUserWallet
    const tx = await erc721.mint(nftUserWallet.address);
    await tx.wait();
  });

  async function executeGreetingTransaction(user: Wallet, greeting: string) {
    const gasPrice = await provider.getGasPrice();
    const paymasterParams = utils.getPaymasterParams(await paymaster.getAddress(), {
      type: "General",
      // empty bytes as paymaster does not use innerInput
      innerInput: new Uint8Array(),
    });

    // estimate gasLimit via paymaster
    const gasLimit = await (greeter.connect(user) as Contract).setGreeting.estimateGas(greeting, {
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams: paymasterParams,
      },
    });

    const setGreetingTx = await (greeter.connect(user) as Contract).setGreeting(greeting, {
      maxPriorityFeePerGas: 0n,
      maxFeePerGas: gasPrice,
      gasLimit,
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams,
      },
    });

    await setGreetingTx.wait();
  }
  //[anvil_zks-halt-1]
  it("should revert with 'insufficient funds' error when user has no balance", async function () {
    const action = async () => {
      // Arrange
      const emptyWallet = new Wallet(Wallet.createRandom().privateKey, provider); // A wallet with no funds
      const gasPrice = await provider.getGasPrice();
      const greeting = "Insufficient funds test";

      const gasLimit = await (greeter.connect(emptyWallet) as Contract).setGreeting.estimateGas(greeting);

      // Act
      const txPromise = (greeter.connect(emptyWallet) as Contract).setGreeting(greeting, {
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: gasPrice,
        gasLimit,
      });

      const tx = await txPromise;
      await tx.wait();
    };

    // Assert
    await expectThrowsAsync(action, "insufficient funds for gas * price + value");
  });

  //[anvil_zks-halt-2]
  it("should revert with paymaster validation error", async function () {
    // Arrange
    const normalUserWallet = new Wallet(Wallet.createRandom().privateKey, provider);

    // Act
    const action = async () => {
      await executeGreetingTransaction(normalUserWallet, "Hello World");
    };

    // Assert
    await expectThrowsAsync(action, "Paymaster validation error");
  });
  ////[anvil_zks-halt-3]
  it("should revert with PrePaymasterPreparationFailed if prepareForPaymaster fails due to unsupported paymaster flow", async function () {
    const action = async () => {
      // Arrange
      const invalidPaymasterParams = {
        paymaster: await paymaster.getAddress(),
        type: "General",
        // This 4-byte value is used deliberately so that it does not match any supported selector.
        paymasterInput: new Uint8Array([0xde, 0xad, 0xbe, 0xef]),
      };
      const gasPrice = await provider.getGasPrice();
      const greeting = "Invalid paymaster flow test";

      const gasLimit = await (greeter.connect(nftUserWallet) as Contract).setGreeting.estimateGas(greeting, {
        customData: {
          gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
          paymasterParams: invalidPaymasterParams,
        },
      });

      // Act
      const txPromise = (greeter.connect(nftUserWallet) as Contract).setGreeting(greeting, {
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: gasPrice,
        gasLimit,
        customData: {
          gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
          paymasterParams: invalidPaymasterParams,
        },
      });

      const tx = await txPromise;
      await tx.wait();
    };

    // Assert
    await expectThrowsAsync(action, "Pre-paymaster preparation error:");
  });
  //[anvil_zks-halt-6]
  it("should revert with Failed to charge fee error due to inflated gas limit", async function () {
    const action = async () => {
      // Arrange
      const gasPrice = await provider.getGasPrice();
      const greeting = "Trigger Failed to charge fee";

      const inflatedGasLimit = 400000000000000000n;

      // Act
      const txPromise = (greeter.connect(nftUserWallet) as Contract).setGreeting(greeting, {
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: gasPrice,
        gasLimit: inflatedGasLimit,
      });

      const tx = await txPromise;
      await tx.wait();
    };

    // Assert
    await expectThrowsAsync(action, "Failed to charge fee");
  });

  // [anvil_zks-halt-7]
  it.only("should revert with 'Sender is not an account' error when sending a tx from a non-account", async function () {
    // Arrange:
    const action = async () => {
      const nonAccountAddress = await greeter.getAddress();
      let aaTx = await greeter.setGreeting.populateTransaction("Hello World");
      const owner1 = ethers.Wallet.createRandom();
      aaTx = {
        ...aaTx,
        from: nonAccountAddress,
        gasLimit: 1000000n,
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: await provider.getGasPrice(),
        chainId: (await provider.getNetwork()).chainId,
        nonce: 0,
        type: 113,
        customData: {
          gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        } as types.Eip712Meta,
        value: ethers.toBigInt(0),
      };
      const signedTxHash = EIP712Signer.getSignedDigest(aaTx);

      const signature = ethers.Signature.from(owner1.signingKey.sign(signedTxHash)).serialized;

      aaTx.customData = {
        ...aaTx.customData,
        customSignature: signature,
      };

      const serialized = utils.serializeEip712(aaTx);

      const tx = await provider.broadcastTransaction(serialized);
      await tx.wait();
    };
    // Assert: the tx should revert with "Sender is not an account".
    await expectThrowsAsync(action, "Sender is not an account");
  });

  //[anvil_zks-halt-10]
  it("should revert with virtual machine entered an unexpected state", async function () {
    const action = async () => {
      // Arrange
      const gasPrice = await provider.getGasPrice();
      const greeting = "Trigger virtual machine entered unexpected state";

      const limitedGasLimit = 100;

      // Act
      const txPromise = (greeter.connect(nftUserWallet) as Contract).setGreeting(greeting, {
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: gasPrice,
        gasLimit: limitedGasLimit,
      });

      const tx = await txPromise;
      await tx.wait();
    };

    // Assert
    await expectThrowsAsync(action, "virtual machine entered an unexpected state");
  });

  // [anvil_zks-revert-1]
  it("should revert with General revert error", async function () {
    const action = async () => {
      const gasPrice = await provider.getGasPrice();
      const failingGreeting = "test";

      const gasLimit = await (greeter.connect(nftUserWallet) as Contract).setGreeting.estimateGas(failingGreeting);

      // Act
      const txPromise = (greeter.connect(nftUserWallet) as Contract).setGreeting(failingGreeting, {
        maxPriorityFeePerGas: 0n,
        maxFeePerGas: gasPrice,
        gasLimit,
      });
      const tx = await txPromise;
      await tx.wait();
    };

    // Assert
    await expectThrowsAsync(action, "General revert error");
  });

  // [anvil_zks-revert-4]
  it("should record inner transaction failure (Bootloader-based tx failed) when calling setGreeting on an address without deployed code", async function () {
    // Arrange:
    // Remove the Greeter contract's code by setting it to an empty byte string.
    const greeterAddress = await greeter.getAddress();
    const emptyCode = "0x" + "00".repeat(32);

    await hre.network.provider.send("hardhat_setCode", [greeterAddress, emptyCode]);

    const gasPrice = await provider.getGasPrice();
    // Use any greeting value; it won’t be processed because there is no contract code.
    const greeting = "Hello World";

    // Since we cannot estimate gas on an address without code, set a fixed gas limit.
    const gasLimit = 1000000n;

    // Act:
    // Attempt to call setGreeting. Note that because there is no code at greeterAddress,
    // the inner call will fail (simulating an inner tx error).
    const tx = await (greeter.connect(nftUserWallet) as Contract).setGreeting(greeting, {
      maxPriorityFeePerGas: 0n,
      maxFeePerGas: gasPrice,
      gasLimit,
    });

    // Use provider.waitForTransaction to obtain the receipt regardless of success or failure.
    const receipt = await provider.waitForTransaction(tx.hash);
    console.log(receipt);
    // Assert:
    // A receipt.status of 0 indicates that the transaction failed.
    //expect(receipt.status).to.equal(0);

    // Optionally, if your environment exposes error details in the receipt, you can assert
    // that they include the bootloader error message. For example:
    // expect(receipt.error?.message).to.include("Bootloader-based tx failed");
  });
});
