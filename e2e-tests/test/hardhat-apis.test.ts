import { expect } from "chai";
import { Wallet } from "zksync-web3";
import { deployContract, getTestProvider } from "../helpers/utils";
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
      startingTimestamp + (numberOfBlocks - 1) * intervalInSeconds * 1000 + 1,
      "Timestamp mismatch"
    );
  });
});

describe("hardhat_impersonateAccount & hardhat_stopImpersonatingAccount", function () {
  it("Should allow transfers of funds without knowing the Private Key", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const beforeBalance = await provider.getBalance(RichAccounts[0].Account);

    // Act
    await provider.send("hardhat_impersonateAccount", [RichAccounts[0].Account]);

    const signer = await ethers.getSigner(RichAccounts[0].Account);
    const tx = {
      to: userWallet.address,
      value: ethers.utils.parseEther("0.42"),
    };

    const recieptTx = await signer.sendTransaction(tx);
    await recieptTx.wait();

    // Assert
    expect((await userWallet.getBalance()).eq(ethers.utils.parseEther("0.42"))).to.true;
    expect((await provider.getBalance(RichAccounts[0].Account)).eq(beforeBalance.sub(ethers.utils.parseEther("0.42"))))
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
    const contractCode = [...ethers.utils.arrayify(artifact.deployedBytecode)];

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

  it("Should update code with a different smart contract", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    expect(await greeter.greet()).to.eq("Hi");
    const artifact = await deployer.loadArtifact("Return5");
    const newContractCode = [...ethers.utils.arrayify(artifact.deployedBytecode)];

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
