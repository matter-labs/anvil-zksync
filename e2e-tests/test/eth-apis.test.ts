import { expect } from "chai";
import { Wallet } from "zksync-web3";
import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";

const provider = getTestProvider();

describe("eth_accounts", function () {
  it("Should return rich accounts", async function () {
    // Arrange
    const richAccounts = RichAccounts.map((ra) => ethers.utils.getAddress(ra.Account)).sort();

    // Act
    const response: string[] = await provider.send("eth_accounts", []);
    const accounts = response.map((addr) => ethers.utils.getAddress(addr)).sort();

    // Assert
    expect(accounts).to.deep.equal(richAccounts);
  });

  it("Should have required fields in transaction receipt", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const tx = await wallet.sendTransaction({
      to: wallet.address,
      value: ethers.utils.parseEther("3"),
    });
    const response = await tx.wait();
    const txHash = response.transactionHash;

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

    // Wait for the transaction to be mined and get the receipt
    const receipt = await provider.waitForTransaction(hash);

    await provider.send("hardhat_stopImpersonatingAccount", [fromAddr]);

    // Assert
    expect(receipt["from"]).to.equal(fromAddr);
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
