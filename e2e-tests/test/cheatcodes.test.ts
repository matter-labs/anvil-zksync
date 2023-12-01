import { expect } from "chai";
import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { RichAccounts } from "../helpers/constants";
import { deployContract, getTestProvider } from "../helpers/utils";

const provider = getTestProvider();

describe("Cheatcodes", function () {
  it("Should test vm.addr", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);

    // Act
    const greeter = await deployContract(deployer, "TestCheatcodes", []);
    const tx = await greeter.testAddr(randomWallet.privateKey, randomWallet.address, {
      gasLimit: 1000000,
    });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
  });

  it("Should test vm.deal", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);

    // Act
    const greeter = await deployContract(deployer, "TestCheatcodes", []);
    const tx = await greeter.testDeal(randomWallet.address, 123456, {
      gasLimit: 1000000,
    });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
  });

  it("Should test vm.etch", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);
    const initialRandomWalletCode = await provider.getCode(randomWallet.address);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const greeter = await deployContract(deployer, "Greeter", ["Hi"]);
    const greeterCode = await provider.getCode(greeter.address);
    const tx = await cheatcodes.testEtch(randomWallet.address, greeterCode);
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
    expect(initialRandomWalletCode).to.eq("0x");
    const finalRandomWalletCode = await provider.getCode(randomWallet.address);
    expect(finalRandomWalletCode).to.eq(greeterCode);
    expect(finalRandomWalletCode).to.not.eq(initialRandomWalletCode);
  });

  it("Should test vm.roll", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const contract = await deployContract(deployer, "TestCheatcodes", []);

    const blockNumber = await provider.getBlockNumber();
    const newBlockNumber = blockNumber + 345;

    // Act
    const tx = await contract.testRoll(newBlockNumber, { gasLimit: 1000000 });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
  });

  it("Should test vm.setNonce and vm.getNonce", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const initialNonce = await provider.getTransactionCount(randomWallet.address);
    const tx = await cheatcodes.testSetNonce(randomWallet.address, 1234, { gasLimit: 1000000 });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
    expect(initialNonce).to.eq(0);
    const finalNonce = await provider.getTransactionCount(randomWallet.address);
    expect(finalNonce).to.eq(1234);
  });

  it("Should test vm.startPrank", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomWallet = Wallet.createRandom().connect(provider);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const tx = await cheatcodes.testStartPrank(randomWallet.address, {
      gasLimit: 10000000,
    });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
  });

  it("Should test vm.startPrank with tx.origin", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);
    const randomMsgSender = Wallet.createRandom().connect(provider);
    const randomTxOrigin = Wallet.createRandom().connect(provider);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const tx1 = await cheatcodes.testStartPrank(randomMsgSender.address, {
      gasLimit: 10000000,
    });
    const receipt1 = await tx1.wait();
    const tx2 = await cheatcodes.testStartPrankWithOrigin(randomMsgSender.address, randomTxOrigin.address, {
      gasLimit: 10000000,
    });
    const receipt2 = await tx2.wait();

    // Assert
    expect(receipt1.status).to.eq(1);
    expect(receipt2.status).to.eq(1);
  });

  it("Should test vm.warp", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    const timeIncreaseInMS = 123;
    let expectedTimestamp: number = await provider.send("config_getCurrentTimestamp", []);
    expectedTimestamp += timeIncreaseInMS;

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const tx = await cheatcodes.testWarp(expectedTimestamp, {
      gasLimit: 1000000,
    });
    expectedTimestamp += 2; // New transaction will add two blocks
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });

  it("Should test vm.store", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey);
    const deployer = new Deployer(hre, wallet);

    // Act
    const cheatcodes = await deployContract(deployer, "TestCheatcodes", []);
    const slot = hre.ethers.constants.HashZero;
    const value = hre.ethers.constants.MaxUint256;
    const tx = await cheatcodes.testStore(slot, value, {
      gasLimit: 10000000,
    });
    const receipt = await tx.wait();

    // Assert
    expect(receipt.status).to.eq(1);
  });
});
