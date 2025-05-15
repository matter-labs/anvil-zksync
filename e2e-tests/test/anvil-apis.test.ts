import { expect } from "chai";
import { Wallet } from "zksync-ethers";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import * as hre from "hardhat";
import * as fs from "node:fs";
import * as path from "node:path";

const provider = getTestProvider();

describe("anvil_setNextBlockBaseFeePerGas", function () {
  it("Should change gas price", async function () {
    const oldBaseFee = await provider.getGasPrice();
    const expectedNewBaseFee = oldBaseFee + 42n;

    // Act
    await provider.send("anvil_setNextBlockBaseFeePerGas", [ethers.toBeHex(expectedNewBaseFee)]);

    // Assert
    const newBaseFee = await provider.getGasPrice();
    expect(newBaseFee).to.equal(expectedNewBaseFee);

    // Revert
    await provider.send("anvil_setNextBlockBaseFeePerGas", [ethers.toBeHex(oldBaseFee)]);
  });

  it("Should produce a block with new gas price", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    const oldBaseFee = await provider.getGasPrice();
    const expectedNewBaseFee = oldBaseFee + 42n;

    // Act
    await provider.send("anvil_setNextBlockBaseFeePerGas", [ethers.toBeHex(expectedNewBaseFee)]);

    const txResponse = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    const txReceipt = await txResponse.wait();
    const newBlock = await provider.getBlock(txReceipt.blockNumber);
    expect(newBlock.baseFeePerGas).to.equal(expectedNewBaseFee);

    // Revert
    await provider.send("anvil_setNextBlockBaseFeePerGas", [ethers.toBeHex(oldBaseFee)]);
  });
});

describe("anvil_setBlockTimestampInterval & anvil_removeBlockTimestampInterval", function () {
  it("Should control timestamp interval between blocks", async function () {
    // Arrange
    const interval = 42;
    let expectedTimestamp: number = await provider.send("config_getCurrentTimestamp", []);
    expectedTimestamp += interval;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Set interval
    await provider.send("anvil_setBlockTimestampInterval", [interval]);

    const txResponse = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    const txReceipt = await txResponse.wait();

    // Assert new block is `interval` apart from start
    const newBlockTimestamp = (await provider.getBlock(txReceipt.blockNumber)).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);

    // Accomodate for virtual block
    expectedTimestamp += interval;

    // Remove interval
    const result: boolean = await provider.send("anvil_removeBlockTimestampInterval", []);
    expect(result);

    const txResponse2 = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    const txReceipt2 = await txResponse2.wait();

    // Assert new block is `1` apart from previous block
    const newBlockTimestamp2 = (await provider.getBlock(txReceipt2.blockNumber)).timestamp;
    expect(newBlockTimestamp2).to.equal(expectedTimestamp + 1);
  });
});

describe("anvil_setLoggingEnabled", function () {
  it("Should disable and enable logging", async function () {
    const logFilePath = process.env.ANVIL_LOG_PATH || path.resolve("../anvil-zksync.log");

    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Act
    await provider.send("anvil_setLoggingEnabled", [false]);

    let logSizeBefore = 0;
    if (fs.existsSync(logFilePath)) {
      logSizeBefore = fs.statSync(logFilePath).size;
    }

    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });

    let logSizeAfter = 0;
    if (fs.existsSync(logFilePath)) {
      logSizeAfter = fs.statSync(logFilePath).size;
    }

    // Reset
    await provider.send("anvil_setLoggingEnabled", [true]);

    // Assert
    expect(logSizeBefore).to.equal(logSizeAfter);
  });
});

describe("anvil_snapshot", function () {
  it("Should return incrementing snapshot ids", async function () {
    const wallet = new Wallet(RichAccounts[6].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    expect(await greeter.greet()).to.eq("Hi");

    // Act
    const snapshotId1: string = await provider.send("anvil_snapshot", []);
    const snapshotId2: string = await provider.send("anvil_snapshot", []);

    // Assert
    expect(await greeter.greet()).to.eq("Hi");
    expect(BigInt(snapshotId2)).to.eq(BigInt(snapshotId1) + 1n);
  });
});

describe("anvil_revert", function () {
  it("Should revert with correct snapshot id", async function () {
    const wallet = new Wallet(RichAccounts[6].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    expect(await greeter.greet()).to.eq("Hi");
    const snapshotId = await provider.send("anvil_snapshot", []);
    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");
    await setGreetingTx.wait();
    expect(await greeter.greet()).to.equal("Hola, mundo!");

    // Act
    const reverted: boolean = await provider.send("anvil_revert", [snapshotId]);

    // Assert
    expect(await greeter.greet()).to.eq("Hi");
    expect(reverted).to.be.true;
  });
});

describe("anvil_increaseTime", function () {
  it("Should increase current timestamp of the node", async function () {
    // Arrange
    const timeIncreaseInSeconds = 13;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    let expectedTimestamp: number = await provider.send("config_getCurrentTimestamp", []);
    expectedTimestamp += timeIncreaseInSeconds;

    // Act
    await provider.send("anvil_increaseTime", [timeIncreaseInSeconds]);

    const txResponse = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    await txResponse.wait();
    expectedTimestamp += 2; // New transaction will add two blocks

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});

describe("anvil_setNextBlockTimestamp", function () {
  it("Should set current timestamp of the node to specific value", async function () {
    // Arrange
    const timeIncreaseInMS = 123;
    let expectedTimestamp: number = await provider.send("config_getCurrentTimestamp", []);
    expectedTimestamp += timeIncreaseInMS;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Act
    await provider.send("anvil_setNextBlockTimestamp", [expectedTimestamp]);

    const txResponse = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    await txResponse.wait();
    expectedTimestamp += 1; // After executing a transaction, the node puts it into a block and increases its current timestamp

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});

describe("anvil_setTime", function () {
  it("Should set current timestamp of the node to specific value", async function () {
    // Arrange
    const timeIncreaseInMS = 123;
    let expectedTimestamp: number = await provider.send("config_getCurrentTimestamp", []);
    expectedTimestamp += timeIncreaseInMS;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Act
    await provider.send("anvil_setTime", [expectedTimestamp]);

    const txResponse = await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("0.1"),
    });
    await txResponse.wait();
    expectedTimestamp += 2; // New transaction will add two blocks

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});

describe("anvil_setBalance", function () {
  it("Should update the balance of an account", async function () {
    // Arrange
    const userWallet = new Wallet(Wallet.createRandom().privateKey).connect(provider);
    const newBalance = ethers.parseEther("42");

    // Act
    await provider.send("anvil_setBalance", [userWallet.address, ethers.toBeHex(newBalance)]);

    // Assert
    const balance = await userWallet.getBalance();
    expect(balance).to.eq(newBalance);
  });
});

describe("anvil_setNonce", function () {
  it("Should update the nonce of an account", async function () {
    // Arrange
    const richWallet = new Wallet(RichAccounts[0].PrivateKey).connect(provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Fund `userWallet` from `richWallet`
    const fundTxResponse = await richWallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("10"),
    });
    await fundTxResponse.wait();

    // Simply asserts that `userWallet` can still send successful transactions
    async function assertCanSendTx() {
      const tx = {
        to: userWallet.address,
        value: ethers.parseEther("0.42"),
      };

      const txResponse = await userWallet.sendTransaction(tx);
      const txReceipt = await txResponse.wait();
      expect(txReceipt!.status).to.equal(1);
    }

    const newNonce = 42;

    // Advance nonce to 42
    await provider.send("anvil_setNonce", [userWallet.address, ethers.toBeHex(newNonce)]);

    // Assert
    expect(await userWallet.getNonce()).to.equal(newNonce);
    await assertCanSendTx();

    // Rollback nonce to 0
    await provider.send("anvil_setNonce", [userWallet.address, ethers.toBeHex(0)]);

    // Assert
    expect(await userWallet.getNonce()).to.equal(0);
    await assertCanSendTx();
  });
});

describe("anvil_mine", function () {
  it("Should mine multiple blocks with a given interval", async function () {
    // Arrange
    const numberOfBlocks = 100;
    const intervalInSeconds = 60;
    const startingBlock = await provider.getBlock("latest");
    const startingTimestamp: number = await provider.send("config_getCurrentTimestamp", []);

    // Act
    await provider.send("anvil_mine", [ethers.toBeHex(numberOfBlocks), ethers.toBeHex(intervalInSeconds)]);

    // Assert
    const latestBlock = await provider.getBlock("latest");
    expect(latestBlock.number).to.equal(startingBlock.number + numberOfBlocks, "Block number mismatch");
    expect(latestBlock.timestamp).to.equal(
      startingTimestamp + (numberOfBlocks - 1) * intervalInSeconds + 1,
      "Timestamp mismatch"
    );
  });
});

describe("anvil_impersonateAccount & anvil_stopImpersonatingAccount", function () {
  it("Should allow transfers of funds without knowing the Private Key", async function () {
    // Arrange
    const userWallet = new Wallet(Wallet.createRandom().privateKey).connect(provider);
    const richAccount = RichAccounts[5].Account;
    const beforeBalance = await provider.getBalance(richAccount);

    // Act
    await provider.send("anvil_impersonateAccount", [richAccount]);

    const signer = await ethers.getSigner(richAccount);
    const tx = {
      to: userWallet.address,
      value: ethers.parseEther("0.42"),
    };

    const recieptTx = await signer.sendTransaction(tx);
    await recieptTx.wait();

    await provider.send("anvil_stopImpersonatingAccount", [richAccount]);

    // Assert
    expect(await userWallet.getBalance()).to.eq(ethers.parseEther("0.42"));
    expect(await provider.getBalance(richAccount)).to.eq(beforeBalance - ethers.parseEther("0.42"));
  });
});

describe("anvil_autoImpersonateAccount", function () {
  it("Should allow transfers of funds without knowing the Private Key", async function () {
    // Arrange
    const userWallet = new Wallet(Wallet.createRandom().privateKey).connect(provider);
    const richAccount = RichAccounts[6].Account;
    const beforeBalance = await provider.getBalance(richAccount);

    // Act
    await provider.send("anvil_autoImpersonateAccount", [true]);

    const signer = await ethers.getSigner(richAccount);
    const tx = {
      to: userWallet.address,
      value: ethers.parseEther("0.42"),
    };

    const recieptTx = await signer.sendTransaction(tx);
    await recieptTx.wait();

    await provider.send("anvil_autoImpersonateAccount", [false]);

    // Assert
    expect(await userWallet.getBalance()).to.eq(ethers.parseEther("0.42"));
    expect(await provider.getBalance(richAccount)).to.eq(beforeBalance - ethers.parseEther("0.42"));
  });
});

describe("anvil_setCode", function () {
  it("Should set code at an address", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const randomWallet = Wallet.createRandom();
    const address = randomWallet.address;
    const artifact = await deployer.loadArtifact("Return5");
    const contractCode = artifact.deployedBytecode;

    // Act
    await provider.send("anvil_setCode", [address, contractCode]);

    // Assert
    const result = await provider.send("eth_call", [
      {
        to: address,
        data: ethers.keccak256(ethers.toUtf8Bytes("value()")).substring(0, 10),
        from: wallet.address,
        gas: "0x1000",
        gasPrice: "0x0ee6b280",
        value: "0x0",
        nonce: "0x1",
      },
      "latest",
    ]);
    expect(BigInt(result)).to.eq(5n);
  });

  it("Should reject invalid code", async function () {
    const action = async () => {
      // Arrange
      const wallet = new Wallet(RichAccounts[0].PrivateKey);
      const deployer = new Deployer(hre, wallet);

      const address = "0x1000000000000000000000000000000000001111";
      const artifact = await deployer.loadArtifact("Return5");
      const contractCode = artifact.deployedBytecode;
      const shortCode = contractCode.slice(0, contractCode.length - 2);

      // Act
      await provider.send("anvil_setCode", [address, shortCode]);
    };

    await expectThrowsAsync(action, "EVM bytecode detected");
  });

  it("Should update code with a different smart contract", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    expect(await greeter.greet()).to.eq("Hi");
    const artifact = await deployer.loadArtifact("Return5");
    const newContractCode = artifact.deployedBytecode;

    // Act
    await provider.send("anvil_setCode", [await greeter.getAddress(), newContractCode]);

    // Assert
    const result = await provider.send("eth_call", [
      {
        to: await greeter.getAddress(),
        data: ethers.keccak256(ethers.toUtf8Bytes("value()")).substring(0, 10),
        from: wallet.address,
        gas: "0x1000",
        gasPrice: "0x0ee6b280",
        value: "0x0",
        nonce: "0x1",
      },
      "latest",
    ]);
    expect(BigInt(result)).to.eq(5n);
  });
});

describe("anvil_setStorageAt", function () {
  it("Should set storage at an address", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = new Wallet(Wallet.createRandom().privateKey).connect(provider);
    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.parseEther("3"),
    });

    const deployer = new Deployer(hre, userWallet);
    const artifact = await deployer.loadArtifact("MyERC20");
    const token = await deployer.deploy(artifact, ["MyToken", "MyToken", 18]);

    const before = await provider.send("eth_getStorageAt", [await token.getAddress(), "0x0", "latest"]);
    expect(BigInt(before)).to.eq(0n);

    const value = ethers.hexlify(ethers.zeroPadValue("0x10", 32));
    await provider.send("anvil_setStorageAt", [await token.getAddress(), "0x0", value]);

    const after = await provider.send("eth_getStorageAt", [await token.getAddress(), "0x0", "latest"]);
    expect(BigInt(after)).to.eq(16n);
  });
});
