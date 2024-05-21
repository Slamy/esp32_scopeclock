// partially inspired by https://github.com/espressif/esp-idf/blob/v5.2.1/components/esp_system/port/soc/esp32/highint_hdl.S
// a lot inspired by https://github.com/liebman/esp32-gps-ntp/blob/master/main/highint5.S


    .set L5_INTR_STACK_SIZE,  12
.data
    .set LX_INTR_A2_OFFSET,   0
    .set LX_INTR_A3_OFFSET,   4
    .set LX_INTR_A4_OFFSET,   8
    .set I2S_INT_CLR_REG,     0x3ff4f018
    .set I2S_INT_ENA_REG,     0x3ff4f014
    .set I2S_INT_RAW_REG,     0x3ff4f00c
    .set GPIO_OUT1_W1TS_REG,  0x3ff44014
    .set DPORT_CPU_INTR_FROM_CPU_3_REG, 0x3ff000e8
 
_lx_intr_stack:
    .space L5_INTR_STACK_SIZE /* This allocates stacks for each individual CPU. */
    
    .macro HANDLE_INTERRUPT_LEVEL2 level
    /* Save A2, A3, A4 so we can use those registers */
    movi    a0, _lx_intr_stack
    s32i    a2, a0, LX_INTR_A2_OFFSET
    s32i    a3, a0, LX_INTR_A3_OFFSET
    s32i    a4, a0, LX_INTR_A4_OFFSET

    // Disable all IRQs as this NMI is quite dangerous
    movi a2, I2S_INT_ENA_REG
    movi a3, 0
    memw
    s32i.n	a3, a2, 0

    // Clear all IRQ flags of I2S by setting bits in I2S_INT_CLR_REG
    movi a2, I2S_INT_CLR_REG
    movi a3, 0xfffff
    memw
    s32i.n	a3, a2, 0

    // Raise SoftwareInterrupt3
    movi a2, DPORT_CPU_INTR_FROM_CPU_3_REG
    movi.n	a3, 1
    memw
    s32i.n	a3, a2, 0

    // Wait for about one microsecond
    movi a0, 100
wait:
    addi a0, a0, -1
    BNEZ a0, wait

    // Set Z Blanking GPIO
    movi a2, GPIO_OUT1_W1TS_REG
    movi.n	a3, 1
    memw
    s32i.n	a3, a2, 0

    movi    a0, _lx_intr_stack
    l32i    a2, a0, LX_INTR_A2_OFFSET
    l32i    a3, a0, LX_INTR_A3_OFFSET
    l32i    a4, a0, LX_INTR_A4_OFFSET
    rsync                                   /* ensure register restored */

    rsr     a0, EXCSAVE\level // restore a0
    rfi \level

    .endm