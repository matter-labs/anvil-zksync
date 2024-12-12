import { expect } from "chai";
import { Wallet, Provider, Contract, utils } from "zksync-ethers";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";
import * as ethers from "ethers";
import * as hre from "hardhat";

import { expectThrowsAsync, getTestProvider } from "../helpers/utils";
import { RichAccounts } from "../helpers/constants";

describe("ERC721GatedPaymaster", function () {
  let provider: Provider;
  let wallet: Wallet;
  let deployer: Deployer;
  let nftUserWallet: Wallet;
  let paymaster: Contract;
  let greeter: Contract;
  let erc721: Contract;

  before(async function () {
    provider = getTestProvider();
    wallet = new Wallet(RichAccounts[0].PrivateKey, provider);
    deployer = new Deployer(hre, wallet);

    // Setup new wallets
    nftUserWallet = new Wallet(Wallet.createRandom().privateKey, provider);

    // Deploy NFT and Paymaster
    let artifact = await deployer.loadArtifact("MyNFT");
    erc721 = await deployer.deploy(artifact, []);
    artifact = await deployer.loadArtifact("ERC721GatedPaymaster");
    paymaster = await deployer.deploy(artifact, [erc721.address]);
    artifact = await deployer.loadArtifact("Greeter");
    greeter = await deployer.deploy(artifact, ["Hi"]);

    // Fund Paymaster
    await provider.send("hardhat_setBalance", [paymaster.address, ethers.toBeHex(ethers.parseEther("10"))]);

    // Assign NFT to nftUserWallet
    const tx = await erc721.mint(nftUserWallet.address);
    await tx.wait();
  });

  async function executeGreetingTransaction(user: Wallet, greeting: string) {
    const gasPrice = await provider.getGasPrice();
    const paymasterParams = utils.getPaymasterParams(await paymaster.getAddress(), {
      type: "General",
      // empty bytes as paymaster does not use innerInput
      innerInput: new Uint8Array(),
    });

    // estimate gasLimit via paymaster
    const gasLimit = await (greeter.connect(user) as Contract).setGreeting.estimateGas(greeting, {
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams: paymasterParams,
      },
    });

    const setGreetingTx = await (greeter.connect(user) as Contract).setGreeting(greeting, {
      maxPriorityFeePerGas: 0n,
      maxFeePerGas: gasPrice,
      gasLimit,
      customData: {
        gasPerPubdata: utils.DEFAULT_GAS_PER_PUBDATA_LIMIT,
        paymasterParams,
      },
    });

    await setGreetingTx.wait();
  }

  it("should not pay for gas fees when user has NFT", async function () {
    // Arrange
    const initialBalance = await nftUserWallet.getBalance();

    // Act
    await executeGreetingTransaction(nftUserWallet, "Hello World");

    // Assert
    expect(await greeter.greet()).to.equal("Hello World");
    const newBalance = await nftUserWallet.getBalance();
    expect(newBalance).to.eql(initialBalance);
  });

  it("should require the user to have the NFT", async function () {
    // Arrange
    const normalUserWallet = new Wallet(Wallet.createRandom().privateKey, provider);

    // Act
    const action = async () => {
      await executeGreetingTransaction(normalUserWallet, "Hello World");
    };

    // Assert
    await expectThrowsAsync(action, "User does not hold the required NFT asset and therefore must for their own gas!");
  });

  it("should allow owner to withdraw all funds", async function () {
    // Arrange
    // Act
    const tx = await (paymaster.connect(wallet) as Contract).withdraw(nftUserWallet.address);
    await tx.wait();

    // Assert
    const finalContractBalance = await provider.getBalance(await paymaster.getAddress());
    expect(finalContractBalance).to.eql(0n);
  });

  it("should prevent non-owners from withdrawing funds", async function () {
    const action = async () => {
      await (paymaster.connect(nftUserWallet) as Contract).withdraw(nftUserWallet.address);
    };

    await expectThrowsAsync(action, "Ownable: caller is not the owner");
  });
});
