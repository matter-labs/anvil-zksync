import { expect } from "chai";
import { Wallet } from "zksync-web3";
import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";

const provider = getTestProvider();

describe("eth_accounts", function () {
  it("Should return legacy rich accounts", async function () {
    // Arrange
    const richAccounts = RichAccounts.map((ra) => ethers.utils.getAddress(ra.Account)).sort();

    // Act
    const response: string[] = await provider.send("eth_accounts", []);
    const accounts = response.map((addr) => ethers.utils.getAddress(addr)).sort();

    // Assert
    expect(accounts).to.include.members(richAccounts);
  });

  it("Should return genesis accounts with sufficient balances", async function () {
    // Arrange
    const genesisAccounts = [
      "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
      "0x90F79bf6EB2c4f870365E785982E1f101E93b906",
      "0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65",
      "0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc",
      "0x976EA74026E726554dB657fA54763abd0C3a0aa9",
      "0x14dC79964da2C08b23698B3D3cc7Ca32193d9955",
      "0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f",
      "0xa0Ee7A142d267C1f36714E4a8F75612F20a79720",
    ];
    const expectedBalance = ethers.utils.parseEther("10000");

    // Act
    const response: string[] = await provider.send("eth_accounts", []);
    const accounts = response.map((addr) => ethers.utils.getAddress(addr));

    // Assert
    expect(accounts).to.include.members(genesisAccounts);

    // Assert
    for (const account of genesisAccounts) {
      const balance = await provider.getBalance(account);
      expect(balance.toString()).to.equal(expectedBalance.toString());
    }
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
