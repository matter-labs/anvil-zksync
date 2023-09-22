import { expect } from "chai";
import { Wallet } from "zksync-web3";
import {
  getTestProvider,
} from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "hardhat";

const provider = getTestProvider();

describe("hardhat_setBalance", function () {
  it("Should update the balance of an account", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const newBalance = ethers.utils.parseEther("42");

    // Act
    await provider.send(
      "hardhat_setBalance",
      [
        userWallet.address,
        newBalance._hex,
      ]
    );

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
    await provider.send(
      "hardhat_setNonce",
      [
        userWallet.address,
        ethers.utils.hexlify(newNonce),
      ]
    );

    // Assert
    const nonce = await userWallet.getNonce();
    expect(nonce).to.equal(newNonce);
  });
});

// TODO: Investigate why deploying a smart contract after this crashes the bootloader/VM
xdescribe("hardhat_mine", function () {
  it("Should mine multiple blocks with a given interval", async function () {
    // Arrange
    const numberOfBlocks = 10;
    const intervalInSeconds = 1000;
    const startingBlock = await provider.getBlock("latest");

    // Act
    await provider.send(
      "hardhat_mine",
      [
        ethers.utils.hexlify(numberOfBlocks),
        ethers.utils.hexlify(intervalInSeconds),
      ]
    );

    // Assert
    const latestBlock = await provider.getBlock("latest");
    expect(latestBlock.number).to.equal(startingBlock.number + numberOfBlocks, "Block number mismatch");
    expect(latestBlock.timestamp).to.equal(startingBlock.timestamp + (numberOfBlocks * intervalInSeconds * 1000), "Timestamp mismatch");
  });
});

// TODO: Run this test once eth_sendTransaction has been implemented or when different tests have been added
xdescribe("hardhat_impersonateAccount & hardhat_stopImpersonatingAccount", function () {
  it("Should allow transfers of funds without knowing the Private Key", async function () {
    // Arrange
    const userWallet = Wallet.createRandom().connect(provider);
    const beforeBalance = await provider.getBalance(RichAccounts[0].Account);

    const actionToTransferWithoutPrivateKey = async () => {

    };

    // Act
    await provider.send(
      "hardhat_impersonateAccount",
      [
        RichAccounts[0].Account,
      ]
    );

    const signer = await ethers.getSigner(RichAccounts[0].Account);
    const tx = {
      to: userWallet.address,
      value: ethers.utils.parseEther("0.42"),
    };

    const recieptTx = await signer.sendTransaction(tx);
    await recieptTx.wait();

    // Assert
    expect(await userWallet.getBalance()).to.equal(ethers.utils.parseEther("0.42"));
    expect(await provider.getBalance(RichAccounts[0].Account)).to.equal(beforeBalance.sub(0.42));
  });
});
