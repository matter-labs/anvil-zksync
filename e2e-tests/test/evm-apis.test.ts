import { expect } from "chai";
import { getTestProvider } from "../helpers/utils";
import { Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";

const provider = getTestProvider();

describe("evm_mine", function () {
  it("Should mine one block", async function () {
    // Arrange
    const startingBlock = await provider.getBlock("latest");

    // Act
    await provider.send("evm_mine", []);

    // Assert
    const latestBlock = await provider.getBlock("latest");
    expect(latestBlock.number).to.equal(startingBlock.number + 1);
  });
});

describe("evm_increaseTime", function () {
  it("Should increase current timestamp of the node", async function () {
    // Arrange
    const timeIncreaseInSeconds = 13;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    let expectedTimestamp: number = await provider.send(
      "config_getCurrentTimestamp",
      []
    );
    expectedTimestamp += timeIncreaseInSeconds * 1000;

    // Act
    await provider.send("evm_increaseTime", [timeIncreaseInSeconds]);

    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.utils.parseEther("0.1"),
    });
    expectedTimestamp += 1; // New transaction will add a second block

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});

describe("evm_setNextBlockTimestamp", function () {
  it("Should set current timestamp of the node to specific value", async function () {
    // Arrange
    const timeIncreaseInMS = 123;
    let expectedTimestamp: number = await provider.send(
      "config_getCurrentTimestamp",
      []
    );
    expectedTimestamp += timeIncreaseInMS;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Act
    await provider.send("evm_setNextBlockTimestamp", [expectedTimestamp]);

    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.utils.parseEther("0.1"),
    });
    expectedTimestamp += 1; // New transaction will add a second block

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});

describe("evm_setTime", function () {
  it("Should set current timestamp of the node to specific value", async function () {
    // Arrange
    const timeIncreaseInMS = 123;
    let expectedTimestamp: number = await provider.send(
      "config_getCurrentTimestamp",
      []
    );
    expectedTimestamp += timeIncreaseInMS;
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);

    // Act
    await provider.send("evm_setTime", [expectedTimestamp]);

    await wallet.sendTransaction({
      to: userWallet.address,
      value: ethers.utils.parseEther("0.1"),
    });
    expectedTimestamp += 1; // New transaction will add a second block

    // Assert
    const newBlockTimestamp = (await provider.getBlock("latest")).timestamp;
    expect(newBlockTimestamp).to.equal(expectedTimestamp);
  });
});
