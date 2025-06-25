import { expect } from "chai";
import { Wallet } from "zksync-ethers";
import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { GenesisAccounts, RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";
import { TransactionResponse } from "zksync-ethers/build/types";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";

const provider = getTestProvider();

describe("eth_accounts", function () {
  it("Should return legacy rich accounts", async function () {
    // Arrange
    const richAccounts = RichAccounts.map((ra) => ethers.getAddress(ra.Account)).sort();

    // Act
    const response: string[] = await provider.send("eth_accounts", []);
    const accounts = response.map((addr) => ethers.getAddress(addr)).sort();

    // Assert
    expect(accounts).to.include.members(richAccounts);
  });

  it("Should return genesis accounts with sufficient balances", async function () {
    // Arrange
    const expectedBalance = ethers.parseEther("10000");

    // Act
    const response: string[] = await provider.send("eth_accounts", []);
    const accounts = response.map((addr) => ethers.getAddress(addr));

    // Assert
    expect(accounts).to.include.members(GenesisAccounts);

    // Assert
    for (const account of GenesisAccounts) {
      const balance = await provider.getBalance(account);
      expect(balance.toString()).to.equal(expectedBalance.toString());
    }
  });

  it("Should have required fields in transaction receipt", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const tx = await wallet.sendTransaction({
      to: wallet.address,
      value: ethers.parseEther("3"),
    });
    const response = await tx.wait();
    const txHash = response.hash;

    // Act
    const receipt = await provider.send("eth_getTransactionReceipt", [txHash]);

    // Assert
    expect(receipt).to.have.property("blockHash");
    expect(receipt).to.have.property("blockNumber");
    expect(receipt).to.have.property("transactionHash");
    expect(receipt).to.have.property("transactionIndex");
    expect(receipt).to.have.property("from");
    expect(receipt).to.have.property("to");
    expect(receipt).to.have.property("cumulativeGasUsed");
    expect(receipt).to.have.property("gasUsed");
    expect(receipt).to.have.property("logs");
    expect(receipt).to.have.property("logsBloom");
    expect(receipt).to.have.property("type");
  });
});

describe("eth_sendTransaction", function () {
  it("Should execute with impersonation", async function () {
    // Arrange
    const fromAddr = "0xE999bb14881e48934A489cC9B35A4f9449EE87fb";
    const toAddr = "0x3355df6d4c9c3035724fd0e3914de96a5a83aaf4";
    const transaction = {
      to: toAddr,
      value: "0x0",
      data: "0xa9059cbb000000000000000000000000981f198286e40f9979274e0876636e9144b8fb8e0000000000000000000000000000000000000000000000000000000000989680",
      from: fromAddr,
    };

    // Act
    await provider.send("hardhat_impersonateAccount", [fromAddr]);

    const hash = await provider.send("eth_sendTransaction", [transaction]);

    // Wait for the transaction to be mined and get the receipt. Used via `TransactionResponse`
    // as the upstream implementation of `Provider::waitForTransaction` has a known race condition:
    // https://github.com/ethers-io/ethers.js/issues/4224.
    const txResponse = new TransactionResponse({ hash, ...transaction }, provider);
    const receipt = await txResponse.wait();

    await provider.send("hardhat_stopImpersonatingAccount", [fromAddr]);

    // Assert
    expect(receipt?.["from"]).to.equal(fromAddr);
  });

  it("Should fail without impersonation", async function () {
    const action = async () => {
      const fromAddr = "0xE999bb14881e48934A489cC9B35A4f9449EE87fb";
      const toAddr = "0x3355df6d4c9c3035724fd0e3914de96a5a83aaf4";
      const transaction = {
        to: toAddr,
        value: "0x0",
        data: "0xa9059cbb000000000000000000000000981f198286e40f9979274e0876636e9144b8fb8e0000000000000000000000000000000000000000000000000000000000989680",
        from: fromAddr,
      };

      await provider.send("eth_sendTransaction", [transaction]);
    };

    await expectThrowsAsync(action, "not allowed to perform transactions");
  });
});

describe("eth_call", function () {
  it("Should execute with state override", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const deployer = new Deployer(hre, wallet);
    const artifact = await deployer.loadArtifact("Fib");
    const bytecode = artifact.bytecode;

    const fromAddr = "0xE999bb14881e48934A489cC9B35A4f9449EE87fb";
    const toAddr = "0xaabbccddeeff00112233445566778899aabbccdd";
    // Data corresponds to `fib(10)` in the Fib contract
    const transaction = {
      to: toAddr,
      value: "0x0",
      data: "0xc6c2ea17000000000000000000000000000000000000000000000000000000000000000a",
      from: fromAddr,
    };

    const overrides = {
      "0xaabbccddeeff00112233445566778899aabbccdd": {
        code: bytecode,
      },
    };

    // Act

    const resp = await provider.send("eth_call", [transaction, "latest", overrides]);

    // Assert
    expect(resp).to.equal("0x0000000000000000000000000000000000000000000000000000000000000059");
  });
});
