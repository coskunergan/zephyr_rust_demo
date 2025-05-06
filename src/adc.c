
#include <zephyr/kernel.h>
#include <zephyr/devicetree.h>
#include <zephyr/drivers/adc.h>

#if !DT_NODE_EXISTS(DT_PATH(zephyr_user)) || \
	!DT_NODE_HAS_PROP(DT_PATH(zephyr_user), io_channels)
#error "No suitable devicetree overlay specified"
#endif

#define DT_SPEC_AND_COMMA(node_id, prop, idx) \
 	ADC_DT_SPEC_GET_BY_IDX(node_id, idx),

const struct adc_dt_spec adc_channels[] =
{
    DT_FOREACH_PROP_ELEM(DT_PATH(zephyr_user), io_channels, DT_SPEC_AND_COMMA)
};

const size_t adc_channels_len = ARRAY_SIZE(adc_channels);

const struct adc_dt_spec *get_adc_dt_spec()
{
    return adc_channels;
}

const size_t get_adc_dt_len()
{
    return adc_channels_len;
}