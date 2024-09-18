import { expect } from "chai";
import { Wallet } from "zksync-web3";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import * as hre from "hardhat";
import { keccak256 } from "ethers/lib/utils";
import { BigNumber } from "ethers";

const provider = getTestProvider();

describe("hardhat_setBalance", function () {
  it("Should update the balance of an account", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const newBalance = ethers.utils.parseEther("42");

    // Act
    await provider.send("hardhat_setBalance", [userWallet.address, newBalance._hex]);

    // Assert
    const balance = await userWallet.getBalance();
    expect(balance.eq(newBalance)).to.true;
  });
});

describe("hardhat_setNonce", function () {
  it("Should update the nonce of an account", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const newNonce = 42;

    // Act
    await provider.send("hardhat_setNonce", [userWallet.address, ethers.utils.hexlify(newNonce)]);

    // Assert
    const nonce = await userWallet.getNonce();
    expect(nonce).to.equal(newNonce);
  });
});

describe("hardhat_mine", function () {
  it("Should mine multiple blocks with a given interval", async function () {
    // Arrange
    const numberOfBlocks = 100;
    const intervalInSeconds = 60;
    const startingBlock = await provider.getBlock("latest");
    const startingTimestamp: number = await provider.send("config_getCurrentTimestamp", []);

    // Act
    await provider.send("hardhat_mine", [
      ethers.utils.hexlify(numberOfBlocks),
      ethers.utils.hexlify(intervalInSeconds),
    ]);

    // Assert
    const latestBlock = await provider.getBlock("latest");
    expect(latestBlock.number).to.equal(startingBlock.number + numberOfBlocks, "Block number mismatch");
    expect(latestBlock.timestamp).to.equal(
      startingTimestamp + (numberOfBlocks - 1) * intervalInSeconds + 1,
      "Timestamp mismatch"
    );
  });
});

describe("hardhat_impersonateAccount & hardhat_stopImpersonatingAccount", function () {
  it("Should allow transfers of funds without knowing the Private Key", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const beforeBalance = await provider.getBalance(RichAccounts[5].Account);

    // Act
    await provider.send("hardhat_impersonateAccount", [RichAccounts[5].Account]);

    const signer = await ethers.getSigner(RichAccounts[5].Account);
    const tx = {
      to: userWallet.address,
      value: ethers.utils.parseEther("0.42"),
    };

    const recieptTx = await signer.sendTransaction(tx);
    await recieptTx.wait();

    await provider.send("hardhat_stopImpersonatingAccount", [RichAccounts[5].Account]);

    // Assert
    expect((await userWallet.getBalance()).eq(ethers.utils.parseEther("0.42"))).to.true;
    expect((await provider.getBalance(RichAccounts[5].Account)).eq(beforeBalance.sub(ethers.utils.parseEther("0.42"))))
      .to.true;
  });
});

describe("hardhat_setCode", function () {
  it("Should set code at an address", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const address = "0x1000000000000000000000000000000000001111";
    const artifact = await deployer.loadArtifact("Return5");
    const contractCode = artifact.deployedBytecode;

    // Act
    await provider.send("hardhat_setCode", [address, contractCode]);

    // Assert
    const result = await provider.send("eth_call", [
      {
        to: address,
        data: keccak256(ethers.utils.toUtf8Bytes("value()")).substring(0, 10),
        from: wallet.address,
        gas: "0x1000",
        gasPrice: "0x0ee6b280",
        value: "0x0",
        nonce: "0x1",
      },
      "latest",
    ]);
    expect(BigNumber.from(result).toNumber()).to.eq(5);
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
      await provider.send("hardhat_setCode", [address, shortCode]);
    };

    await expectThrowsAsync(action, "bytes must be divisible by 32");
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
    await provider.send("hardhat_setCode", [greeter.address, newContractCode]);

    // Assert
    const result = await provider.send("eth_call", [
      {
        to: greeter.address,
        data: keccak256(ethers.utils.toUtf8Bytes("value()")).substring(0, 10),
        from: wallet.address,
        gas: "0x1000",
        gasPrice: "0x0ee6b280",
        value: "0x0",
        nonce: "0x1",
      },
      "latest",
    ]);
    expect(BigNumber.from(result).toNumber()).to.eq(5);
  });
});

describe("hardhat_reset", function () {
  it("should return the correct block number after a hardhat_reset", async function () {
    const oldBlockNumber = await provider.send("eth_blockNumber", []);

    await provider.send("evm_mine", []);
    await provider.send("evm_mine", []);

    const blockNumber = await provider.send("eth_blockNumber", []);
    expect(BigNumber.from(blockNumber).toNumber()).to.be.eq(BigNumber.from(oldBlockNumber).toNumber() + 2);

    await provider.send("hardhat_reset", []);
    const newBlockNumber = await provider.send("eth_blockNumber", []);
    expect(BigNumber.from(newBlockNumber).toNumber()).to.be.eq(0);
  });
});

describe("hardhat_setStorageAt", function () {
  it("Should set storage at an address", async function () {
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.utils.parseEther("3"),
    });

    const deployer = new Deployer(hre, userWallet);
    const artifact = await deployer.loadArtifact("MyERC20");
    const token = await deployer.deploy(artifact, ["MyToken", "MyToken", 18]);

    const before = await provider.send("eth_getStorageAt", [token.address, "0x0", "latest"]);
    expect(BigNumber.from(before).toNumber()).to.eq(0);

    const value = ethers.utils.hexlify(ethers.utils.zeroPad("0x10", 32));
    await provider.send("hardhat_setStorageAt", [token.address, "0x0", value]);

    const after = await provider.send("eth_getStorageAt", [token.address, "0x0", "latest"]);
    expect(BigNumber.from(after).toNumber()).to.eq(16);
  });
});
