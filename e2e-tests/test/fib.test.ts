import { Wallet } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import { RichAccounts } from "../helpers/constants";
import { deployContract, expectThrowsAsync, getTestProvider } from "../helpers/utils";

const provider = getTestProvider();

describe("Test Fib error flags", function () {
  it("Should print to the console NOT ENOUGH ERGS", async function () {
    const action = async () => {
      const wallet = new Wallet(RichAccounts[0].PrivateKey, provider);

      const deployer = new Deployer(hre, wallet);
      const fib = await deployContract(deployer, "Fib");
      await fib.fib(100);
    };

    // This is expected to throw and the console is expected to show:
    //   XX:YY:ZZ ERROR !! Got error flags:
    //   XX:YY:ZZ ERROR NOT ENOUGH ERGS
    await expectThrowsAsync(action, "call revert exception");
  });
});
