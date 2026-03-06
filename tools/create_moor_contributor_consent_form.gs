function createMoorContributorConsentForm() {
  const claUrl = 'https://codeberg.org/timbran/moor/src/branch/main/CLA.md';

  const form = FormApp.create('mooR Contributor Licensing Consent');
  form.setDescription(
    'This form is for past contributors to mooR.\n\n' +
    'mooR is updating its project licensing and contributor paperwork. ' +
    'This form records your written consent regarding your past contributions to the project.\n\n' +
    'Please read the current mooR Contributor License Agreement before submitting:\n' +
    claUrl + '\n\n' +
    'If you agree, this form records that your past contributions to mooR may be used under those terms.'
  );

  form.setCollectEmail(true);
  form.setConfirmationMessage(
    'Your response has been recorded. Thank you.'
  );

  form.addTextItem()
    .setTitle('Full legal name')
    .setRequired(true);

  form.addTextItem()
    .setTitle('Git / Codeberg / GitHub handle')
    .setRequired(false);

  form.addMultipleChoiceItem()
    .setTitle('Authority to consent')
    .setChoiceValues([
      'I am contributing on my own behalf',
      'I am contributing on behalf of an employer or organization and I have authority to agree',
      'I am not sure'
    ])
    .setRequired(true);

  form.addSectionHeaderItem()
    .setTitle('Consent Statement')
    .setHelpText(
      'By submitting this form, I agree that my past contributions to the mooR project may be used by ' +
      'Ryan Daum, operating as Timbran Consulting, under the terms of the mooR Contributor License Agreement, ' +
      'including the right to use, distribute, sublicense, and relicense those contributions as part of mooR ' +
      'or derivative works of mooR.'
    );

  form.addMultipleChoiceItem()
    .setTitle('Have you read the mooR Contributor License Agreement?')
    .setChoiceValues([
      'Yes',
      'No'
    ])
    .setRequired(true);

  form.addMultipleChoiceItem()
    .setTitle('Do you agree to the consent statement above?')
    .setChoiceValues([
      'Yes',
      'No'
    ])
    .setRequired(true);

  form.addTextItem()
    .setTitle('Typed name')
    .setHelpText('Type your full name again as your electronic signature.')
    .setRequired(true);

  form.addDateItem()
    .setTitle('Date')
    .setRequired(true);

  form.addParagraphTextItem()
    .setTitle('Notes')
    .setRequired(false);

  Logger.log('Edit URL: ' + form.getEditUrl());
  Logger.log('Published URL: ' + form.getPublishedUrl());
}
