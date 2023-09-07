import { expect } from 'chai';
import { Wallet, Contract, Provider } from 'zksync-web3';
import * as hre from 'hardhat';
import { Deployer } from '@matterlabs/hardhat-zksync-deploy';
import { ethers } from 'ethers';

const RICH_WALLET_PK =
  '0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110';

async function deployGreeter(deployer: Deployer): Promise<Contract> {
  const artifact = await deployer.loadArtifact('Greeter');
  return await deployer.deploy(artifact, ['Hi']);
}

async function fundAccount(
  wallet: ethers.Wallet,
  address: string,
  amount: string,
) {
  await (
    await wallet.sendTransaction({
      to: address,
      value: ethers.utils.parseEther(amount),
    })
  ).wait();
}

describe('Greeter', function () {
  it("Should return the new greeting once it's changed", async function () {
    const wallet = new Wallet(RICH_WALLET_PK);
    const deployer = new Deployer(hre, wallet);

    const greeter = await deployGreeter(deployer);

    expect(await greeter.greet()).to.eq('Hi');

    const setGreetingTx = await greeter.setGreeting('Hola, mundo!');
    // wait until the transaction is mined
    await setGreetingTx.wait();

    expect(await greeter.greet()).to.equal('Hola, mundo!');
  });

  it("should prevent non-owners from setting greeting", async function () {
    let errorThrown = false;
    try {
        const provider = new Provider("http://127.0.0.1:8011");
        const wallet = new Wallet(RICH_WALLET_PK, provider);
        const deployer = new Deployer(hre, wallet);

        // setup user wallet
        const userWallet = Wallet.createRandom().connect(provider);
        await fundAccount(wallet, userWallet.address, "3");
        
        // deploy Greeter contract
        const artifact = await deployer.loadArtifact('Greeter');
        const greeter = await deployer.deploy(artifact, ["Hello, world!"]);
        
        // should revert
        const tx = await greeter.connect(userWallet).setGreeting("Hola, mundo!");
        await tx.wait();
    } catch (e) {
      expect(e.message).to.include("Ownable: caller is not the owner");
      errorThrown = true;
    }

    expect(errorThrown).to.be.true;
  });
});
