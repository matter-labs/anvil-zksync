import { expect } from "chai";
import { getTestProvider } from "../helpers/utils";
import { Wallet } from "zksync-web3";
import { RichAccounts } from "../helpers/constants";
import { ethers } from "ethers";
import { TransactionRequest } from "zksync-web3/build/src/types";

const provider = getTestProvider();

interface Fee {
  gas_limit: ethers.BigNumber;
  gas_per_pubdata_limit: ethers.BigNumber;
  max_fee_per_gas: ethers.BigNumber;
  max_priority_fee_per_gas: ethers.BigNumber;
}

// TODO: Investigate why this test is failing for CI vs local development
xdescribe("zks_estimateFee", function () {
  it("Should return fee estimation data for transfer of 1 ETH", async function () {
    // Arrange
    const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    const userWallet = Wallet.createRandom().connect(provider);
    const transaction: TransactionRequest = {
      from: wallet.address,
      to: userWallet.address,
      value: ethers.utils.parseEther("1")._hex,
    };

    // Act
    const response: Fee = await provider.send("zks_estimateFee", [transaction]);

    // Assert
    expect(ethers.BigNumber.from(response.gas_limit)).to.eql(ethers.BigNumber.from("1233024"), "Unexpected gas_limit");
    expect(ethers.BigNumber.from(response.gas_per_pubdata_limit)).to.eql(
      ethers.BigNumber.from("4080"),
      "Unexpected gas_per_pubdata_limit"
    );
    expect(ethers.BigNumber.from(response.max_fee_per_gas)).to.eql(
      ethers.BigNumber.from("250000000"),
      "Unexpected max_fee_per_gas"
    );
    expect(ethers.BigNumber.from(response.max_priority_fee_per_gas)).to.eql(
      ethers.BigNumber.from("0"),
      "Unexpected max_priority_fee_per_gas"
    );
  });
});

describe("zks_getTokenPrice", function () {
  it("Should return fake token Price for ETH", async function () {
    // Arrange
    const ethAddress = "0x0000000000000000000000000000000000000000";

    // Act
    const response: string = await provider.send("zks_getTokenPrice", [ethAddress]);

    // Assert
    expect(response).to.equal("1500");
  });
});
