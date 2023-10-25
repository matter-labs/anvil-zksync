import { expect } from "chai";
import { Wallet, Provider, Contract, utils } from "zksync-web3";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import * as ethers from "ethers";
import * as hre from "hardhat";

import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";

describe("ERC20FixedPaymaster", function () {
  let provider: Provider;
  let richWallet: Wallet;
  let deployer: Deployer;
  let userWallet: Wallet;
  let paymaster: Contract;
  let greeter: Contract;
  let token: Contract;

  before(async function () {
    provider = getTestProvider();
    richWallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    deployer = new Deployer(hre, richWallet);

    // Setup new wallet
    const emptyWallet = Wallet.createRandom();
    userWallet = new Wallet(emptyWallet.privateKey, provider);

    // Deploy ERC20 token, Paymaster, and Greeter
    let artifact = await deployer.loadArtifact("MyERC20");
    token = await deployer.deploy(artifact, ["MyToken", "MyToken", 18]);
    artifact = await deployer.loadArtifact("ERC20FixedPaymaster");
    paymaster = await deployer.deploy(artifact, [token.address]);
    artifact = await deployer.loadArtifact("Greeter");
    greeter = await deployer.deploy(artifact, ["Hi"]);

    // Fund Paymaster
    await provider.send("hardhat_setBalance", [paymaster.address, ethers.utils.parseEther("10")._hex]);
  });

  async function executeGreetingTransaction(user: Wallet, greeting: string) {
    const gasPrice = await provider.getGasPrice();
    const token_address = token.address.toString();

    const paymasterParams = utils.getPaymasterParams(paymaster.address, {
      type: "ApprovalBased",
      token: token_address,
      minimalAllowance: ethers.BigNumber.from(1),
      // empty bytes as testnet paymaster does not use innerInput
      innerInput: new Uint8Array(),
    });

    // Estimate gasLimit via paymaster
    const gasLimit = await greeter.connect(user).estimateGas.setGreeting(greeting, {
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams,
      },
    });

    const setGreetingTx = await greeter.connect(user).setGreeting(greeting, {
      maxPriorityFeePerGas: ethers.BigNumber.from(0),
      maxFeePerGas: gasPrice,
      gasLimit,
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams,
      },
    });

    await setGreetingTx.wait();
  }

  it("user with MyERC20 token can update message for free", async function () {
    // Arrange
    const initialMintAmount = ethers.utils.parseEther("3");
    const success = await token.mint(userWallet.address, initialMintAmount);
    await success.wait();

    const userInitialTokenBalance = await token.balanceOf(userWallet.address);
    const userInitialETHBalance = await userWallet.getBalance();
    const initialPaymasterBalance = await provider.getBalance(paymaster.address);

    // Act
    await executeGreetingTransaction(userWallet, "Hola, mundo!");

    // Assert
    const finalETHBalance = await userWallet.getBalance();
    const finalUserTokenBalance = await token.balanceOf(userWallet.address);
    const finalPaymasterBalance = await provider.getBalance(paymaster.address);

    expect(await greeter.greet()).to.equal("Hola, mundo!");
    expect(initialPaymasterBalance.gt(finalPaymasterBalance)).to.be.true;
    expect(userInitialETHBalance).to.eql(finalETHBalance);
    expect(userInitialTokenBalance.gt(finalUserTokenBalance)).to.be.true;
  });

  it("should allow owner to withdraw all funds", async function () {
    // Arrange
    // Act
    const tx = await paymaster.connect(richWallet).withdraw(userWallet.address);
    await tx.wait();

    // Assert
    const finalContractBalance = await provider.getBalance(paymaster.address);
    expect(finalContractBalance).to.eql(ethers.BigNumber.from(0));
  });

  it("should prevent non-owners from withdrawing funds", async function () {
    const action = async () => {
      await paymaster.connect(userWallet).withdraw(userWallet.address);
    };

    await expectThrowsAsync(action, "Ownable: caller is not the owner");
  });
});
