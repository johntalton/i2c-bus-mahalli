import { Mahalli, MAX_ADDR, MIN_ADDR } from './index.js'

/** @import { BusOptions } from './index.js' */
/** @import { I2CBus, I2CAddress, I2CCommand, I2CBufferSource, I2CReadResult, I2CWriteResult } from '@johntalton/and-other-delights' */

/**
 * @overload
 * @param {ArrayBufferLike} buffer
 * @returns {buffer is ArrayBuffer}
 */
/**
 * @overload
 * @param {ArrayBufferView<ArrayBufferLike>} buffer
 * @returns {buffer is ArrayBufferView<ArrayBuffer>}
 */
/**
 * @overload
 * @param {ArrayBufferLike | ArrayBufferView<ArrayBufferLike>} buffer
 * @returns {buffer is ArrayBuffer|ArrayBufferView<ArrayBuffer>}
 */
/**
 * @param {ArrayBufferLike|ArrayBufferView} buffer
 * @returns {buffer is ArrayBuffer|ArrayBufferView<ArrayBuffer>}
 */
export function isNonSharedArrayBuffer(buffer) {
	if(typeof SharedArrayBuffer === 'undefined') { return true }

	if(ArrayBuffer.isView(buffer)) {
		return !(buffer.buffer instanceof SharedArrayBuffer)
	}

	return !(buffer instanceof SharedArrayBuffer)
}

export const DEV_FS_PREFIX = '/dev/i2c-'

/**
 * @implements {I2CBus}
 */
export class I2CBusMahalli {
	#mahalli

	name = 'I2CBusMahalli'
	supportsScan = true
	supportsMultiByteDataAddress = false

	/**
	 * @param {number} busNumber
	 * @param {BusOptions} [options]
	 */
	constructor(busNumber, options) {
		this.#mahalli = new Mahalli(`${DEV_FS_PREFIX}${busNumber}`, options)
	}

	[Symbol.dispose]() {
		this.close()
	}

	close() {
		this.#mahalli.close()
	}

	/**
	 * @returns {Promise<I2CAddress[]>}
	 */
	async scan() {
		return this.#mahalli.scan({
			startAddr: MIN_ADDR,
			endAddr: MAX_ADDR
		})
	}

	/**
	 * @param {I2CAddress} address
	 * @param {number} byteValue
	 * @returns {Promise<void>}
	 */
	async sendByte(address, byteValue) {
		throw new Error('Method not implemented.')
	}

	/**
	 * @param {I2CAddress} address
	 * @param {I2CCommand} cmd
	 * @param {number} length
	 * @param {I2CBufferSource} [targetBuffer]
	 * @returns {Promise<I2CReadResult>}
	 */
	async readI2cBlock(address, cmd, length, targetBuffer) {
		if(targetBuffer !== undefined) { throw new Error('BYOB buffers not supported') }
		if(Array.isArray(cmd)) { throw new Error('multi-byte command not supported') }
		return this.#mahalli.readI2CBlock(address, cmd, length)
	}

	/**
	 * @param {I2CAddress} address
	 * @param {I2CCommand} cmd
	 * @param {number} length
	 * @param {I2CBufferSource} buffer
	 * @returns {Promise<I2CWriteResult>}
	 */
	async writeI2cBlock(address, cmd, length, buffer) {
		if(Array.isArray(cmd)) { throw new Error('multi-byte command not supported') }

		if(!isNonSharedArrayBuffer(buffer)) { throw new Error('SharedArrayBuffer not supported') }
		const view = ArrayBuffer.isView(buffer) ?
			buffer :
			new Uint8Array(buffer, 0, length)
		if(view.byteLength !== length) { throw new Error('length miss-match') }

		const result = await this.#mahalli.writeI2CBlock(address, cmd, view)
		return {
			...result,
			buffer
		}
	}

	/**
	 * @param {I2CAddress} address
	 * @param {number} length
	 * @param {I2CBufferSource} [targetBuffer]
	 * @returns {Promise<I2CReadResult>}
	 */
	async i2cRead(address, length, targetBuffer) {
		if(targetBuffer !== undefined) { throw new Error('BYOB buffers not supported') }
		return this.#mahalli.i2CRead(address, length)
	}

	/**
	 * @param {I2CAddress} address
	 * @param {number} length
	 * @param {I2CBufferSource} buffer
	 * @returns {Promise<I2CWriteResult>}
	 */
	async i2cWrite(address, length, buffer) {
		if(!isNonSharedArrayBuffer(buffer)) { throw new Error('SharedArrayBuffer not supported') }
		const view = ArrayBuffer.isView(buffer) ?
			buffer :
			new Uint8Array(buffer, 0, length)
		if(view.byteLength !== length) { throw new Error('length miss-match') }

		const result = await this.#mahalli.i2CWrite(address, view)
		return {
			...result,
			buffer
		}
	}
}
