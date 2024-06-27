import { expect } from "chai";
import { Wallet } from "zksync-web3";
import { getTestProvider } from "../helpers/utils";
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
