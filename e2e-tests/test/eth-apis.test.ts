import { expect } from "chai";
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
});
