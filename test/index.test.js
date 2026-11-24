import test from 'node:test'
import assert from 'node:assert/strict'

import { I2CBusMahalli } from '@johntalton/i2c-bus-mahalli'

test('exports I2CBusMahalli class', () => {
  assert.equal(typeof I2CBusMahalli, 'function')
});

test('throws expected error when opening bus device', () => {
  if (process.platform === 'linux') {
    assert.throws(
      () => new I2CBusMahalli('/dev/i2c-nonexistent-99'),
      /Failed to open \/dev\/i2c-nonexistent-99/,
    );
  } else {
    assert.throws(
      () => new I2CBusMahalli('/dev/i2c-nonexistent-99'),
      /i2cdev is only supported on Linux/,
    )
  }
})
