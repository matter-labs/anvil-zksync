import { expect } from "chai";
import {
  getTestProvider,
} from "../helpers/utils";

const provider = getTestProvider();

// TODO: Investigate why deploying a smart contract after this crashes the bootloader/VM
xdescribe("evm_mine", function () {
  it("Should mine one block", async function () {
    // Arrange
    const startingBlock = await provider.getBlock("latest");

    // Act
    await provider.send(
      "evm_mine",
      []
    );

    // Assert
    const latestBlock = await provider.getBlock("latest");
    expect(latestBlock.number).to.equal(startingBlock.number + 1);
  });
});
